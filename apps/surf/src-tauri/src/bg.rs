//! Per-extension background thread. rquickjs is !Send, so each
//! extension's QuickJS context lives on its own OS thread and talks to
//! the rest of the browser through channels (MPSC in, outgoing events
//! forwarded through the router on the main side).

use std::collections::HashMap;
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;

use surf_ext_host::api::{ApiError, ApiHost, ExtContext};
use tauri::Manager;
use surf_ext_host::router::{Envelope, Endpoint, MessageBody};
use surf_ext_host::runtime::{ExtRuntime, RuntimeLimits};
use surf_ext_host::{ExtensionId, InstalledExtension};

pub enum BgMsg {
    Envelope(Envelope),
    Reload(InstalledExtension),
}

pub struct BgHandle {
    pub tx: Sender<BgMsg>,
    /// Kept so the thread can be joined on shutdown (unused in M2).
    #[allow(dead_code)]
    pub join: std::thread::JoinHandle<()>,
}

/// Everything the background thread needs from the browser side.
pub struct BgHost {
    pub app: tauri::AppHandle,
    pub api: Arc<ApiHost>,
    pub permission: Arc<std::sync::Mutex<surf_ext_host::permission::PermissionManager>>,
}

impl BgHost {
    fn call_api(&self, ext: &ExtensionId, module: &str, method: &str, args: &serde_json::Value) -> Result<serde_json::Value, ApiError> {
        let mut pm = self.permission.lock().unwrap();
        if !pm.ensure(ext, &surf_ext_host::permission::PermissionRequest::Api(module.to_owned())) {
            return Err(ApiError::Denied(module.to_owned()));
        }
        drop(pm);
        let ctx = ExtContext { ext: ext.clone(), tab: None };
        self.api.call(&ctx, module, method, args)
    }
}

pub fn spawn(app: tauri::AppHandle, ext: InstalledExtension) -> BgHandle {
    let (tx, rx) = mpsc::channel::<BgMsg>();
    let id = ext.id.clone();
    let api: Arc<ApiHost> = Arc::new(crate::api_impl::build_host(app.clone()));
    let permission = app.state::<crate::state::BrowserState>().permission.clone();
    let join = std::thread::Builder::new()
        .name(format!("ext-{}", id.0))
        .spawn(move || {
            let host = BgHost { app, api, permission };
            run_ext(id, ext, rx, host);
        })
        .expect("spawn ext thread");
    BgHandle { tx, join }
}

fn run_ext(id: ExtensionId, ext: InstalledExtension, rx: std::sync::mpsc::Receiver<BgMsg>, host: BgHost) {
    let runtime = match ExtRuntime::new(id.clone(), RuntimeLimits::default()) {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("ext {} runtime failed: {e}", id.0);
            return;
        }
    };
    // Load background scripts. Restarting on manifest change: the thread
    // exits and setup re-spawns on the Reload message.
    let mut active = ext;
    loop {
        for script in active.manifest.background_scripts() {
            let path = active.root.join(script);
            match std::fs::read_to_string(&path) {
                Ok(code) => {
                    if let Err(e) = runtime.eval(&code) {
                        eprintln!("ext {} background script error: {e}", id.0);
                    }
                }
                Err(e) => eprintln!("ext {} background script unreadable: {e}", id.0),
            }
        }

        loop {
            let msg = match rx.recv_timeout(std::time::Duration::from_millis(300)) {
                Ok(m) => m,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    forward_outgoing(&host, &id, &runtime);
                    continue;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
            };
            match msg {
                BgMsg::Reload(new_ext) => {
                    active = new_ext;
                    break;
                }
                BgMsg::Envelope(env) => {
                let env_from = env.from.clone();
                match env.body {
                    MessageBody::Event { name, payload } => {
                        let _ = runtime.dispatch_event(&name, &payload);
                    }
                    MessageBody::ApiCall { module, method, payload } => {
                        let result = host.call_api(&id, &module, &method, &payload.clone());
                        if let Some(reply_to) = env_from_matches(&env_from) {
                            let body = match result {
                                Ok(v) => MessageBody::Response { ok: v },
                                Err(e) => MessageBody::Error { message: e.to_string() },
                            };
                            let _ = host.app.state::<crate::state::BrowserState>().router.lock().unwrap().send(Envelope {
                                from: Endpoint::Extension(id.clone()),
                                to: reply_to,
                                body,
                            });
                        }
                    }
                    MessageBody::Response { .. } | MessageBody::Error { .. } => {}
                }
                }
            }
            forward_outgoing(&host, &id, &runtime);
        }
        // Loop continues into script reload for the new manifest.
    }
}

/// Forward script-emitted events: "cs" targets one content tab, anything
/// else goes to the host.
fn forward_outgoing(host: &BgHost, id: &ExtensionId, runtime: &ExtRuntime) {
    {
            for out in runtime.drain_outgoing() {
                let payload = serde_json::from_str::<serde_json::Value>(&out.payload)
                    .unwrap_or(serde_json::Value::String(out.payload.clone()));
                // Host API bridge: the shim's surf.callApi lands here as __api.
                if out.name == "__api" {
                    let obj = payload.as_object().cloned().unwrap_or_default();
                    let call_id = obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_owned();
                    let module = obj.get("module").and_then(|v| v.as_str()).unwrap_or("").to_owned();
                    let method = obj.get("method").and_then(|v| v.as_str()).unwrap_or("").to_owned();
                    let args = obj.get("args").cloned().unwrap_or(serde_json::Value::Null);
                    let result = host.call_api(id, &module, &method, &args);
                    let reply = match result {
                        Ok(v) => serde_json::json!({ "ok": v }),
                        Err(e) => serde_json::json!({ "error": e.to_string() }),
                    };
                    let event = format!("__apiResult:{call_id}");
                    if let Err(e) = runtime.dispatch_event(&event, &reply) {
                        eprintln!("[ext-bg] {event} dispatch error: {e}");
                    }
                    continue;
                }
                match out.name.as_str() {
                    "cs" => {
                        let tab = payload.get("tab").and_then(|v| v.as_u64()).unwrap_or(0);
                        let name = payload.get("name").and_then(|v| v.as_str()).unwrap_or("").to_owned();
                        let inner = payload.get("payload").cloned().unwrap_or(serde_json::Value::Null);
                        let _ = host.app.state::<crate::state::BrowserState>().router.lock().unwrap().send(Envelope {
                            from: Endpoint::Extension(id.clone()),
                            to: Endpoint::Content(surf_core::tabs::TabId(tab)),
                            body: MessageBody::Event { name, payload: inner },
                        });
                    }
                    other => {
                        let _ = host.app.state::<crate::state::BrowserState>().router.lock().unwrap().send(Envelope {
                            from: Endpoint::Extension(id.clone()),
                            to: Endpoint::Host,
                            body: MessageBody::Event { name: other.to_owned(), payload },
                        });
                    }
                }
            }
    }
}

fn env_from_matches(from: &Endpoint) -> Option<Endpoint> {
    if matches!(from, Endpoint::Content(_)) {
        Some(from.clone())
    } else {
        None
    }
}

/// Registry of running background threads.
#[derive(Default)]
pub struct BgRegistry {
    threads: HashMap<ExtensionId, BgHandle>,
}

impl BgRegistry {
    pub fn start(&mut self, app: &tauri::AppHandle, ext: InstalledExtension) {
        let id = ext.id.clone();
        if let Some(existing) = self.threads.get(&id) {
            let _ = existing.tx.send(BgMsg::Reload(ext));
            return;
        }
        let handle = spawn(app.clone(), ext);
        self.threads.insert(id, handle);
    }

    pub fn sender(&self, id: &ExtensionId) -> Option<Sender<BgMsg>> {
        self.threads.get(id).map(|h| h.tx.clone())
    }

    pub fn ids(&self) -> Vec<ExtensionId> {
        self.threads.keys().cloned().collect()
    }
}
