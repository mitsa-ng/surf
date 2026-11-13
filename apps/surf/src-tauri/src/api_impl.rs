//! Real ApiModule implementations living in the shell, where the DB,
//! tab manager, and network are reachable. surf-ext-host defines the
//! trait; these impls are the product surface.

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use surf_core::tabs::TabId;
use surf_ext_host::api::{ApiError, ApiHost, ApiModule, ExtContext};
use tauri::Manager;

use crate::manager;
use crate::state::{self, BrowserState};

fn str_arg(args: &Value, key: &str) -> Result<String, ApiError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .ok_or_else(|| ApiError::UnknownMethod { module: "args".into(), method: key.to_owned() })
}

/// chrome.storage.local equivalent, namespaced per extension.
pub struct StorageModule {
    db: Arc<Mutex<surf_core::storage::Db>>,
}

impl StorageModule {
    pub fn new(db: Arc<Mutex<surf_core::storage::Db>>) -> Self {
        Self { db }
    }
}

impl ApiModule for StorageModule {
    fn name(&self) -> &'static str {
        "storage"
    }

    fn call(&self, ctx: &ExtContext, method: &str, args: &Value) -> Result<Value, ApiError> {
        let db = self.db.lock().unwrap();
        match method {
            "get" => {
                let key = str_arg(args, "key")?;
                match db.ext_kv_get(&ctx.ext.0, &key).map_err(|e| ApiError::Denied(e.to_string()))? {
                    Some(v) => Ok(json!({ "key": key, "value": v })),
                    None => Ok(Value::Null),
                }
            }
            "set" => {
                let key = str_arg(args, "key")?;
                let value = str_arg(args, "value")?;
                db.ext_kv_set(&ctx.ext.0, &key, &value).map_err(|e| ApiError::Denied(e.to_string()))?;
                Ok(json!({ "ok": true }))
            }
            "remove" => {
                let key = str_arg(args, "key")?;
                db.ext_kv_remove(&ctx.ext.0, &key).map_err(|e| ApiError::Denied(e.to_string()))?;
                Ok(json!({ "ok": true }))
            }
            "keys" => db
                .ext_kv_keys(&ctx.ext.0)
                .map(|keys| json!({ "keys": keys }))
                .map_err(|e| ApiError::Denied(e.to_string())),
            _ => Err(ApiError::UnknownMethod { module: self.name().to_owned(), method: method.to_owned() }),
        }
    }
}

/// tabs.* over the Rust tab model. create/activate/close are desktop-only
/// for now (mobile multi-tab is M2); list works everywhere.
pub struct TabsModule {
    app: tauri::AppHandle,
}

impl TabsModule {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

impl ApiModule for TabsModule {
    fn name(&self) -> &'static str {
        "tabs"
    }

    fn call(&self, _ctx: &ExtContext, method: &str, args: &Value) -> Result<Value, ApiError> {
        let state = self.app.state::<BrowserState>();
        match method {
            "list" => {
                let model = state::lock(&state.model);
                Ok(json!(model.list()))
            }
            "create" => {
                #[cfg(not(desktop))]
                {
                    let _ = args;
                    Err(ApiError::Denied("mobile multi-tab lands with instance-swap (M2)".into()))
                }
                #[cfg(desktop)]
                {
                    let url = args.get("url").and_then(|v| v.as_str());
                    let window = self
                        .app
                        .get_window(crate::state::MAIN_LABEL)
                        .ok_or_else(|| ApiError::Denied("main window missing".into()))?;
                    let parsed = match url {
                        Some(u) => Some(
                            url::Url::parse(u)
                                .map_err(|e| ApiError::Denied(format!("bad url: {e}")))?,
                        ),
                        None => None,
                    };
                    let id = manager::open_tab(&self.app, &window, parsed);
                    Ok(json!({ "id": id.0 }))
                }
            }
            "activate" => {
                #[cfg(not(desktop))]
                {
                    let _ = args;
                    Err(ApiError::Denied("mobile multi-tab lands with instance-swap (M2)".into()))
                }
                #[cfg(desktop)]
                {
                    let id = args.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                    manager::activate_tab(&self.app, TabId(id)).map_err(|e| ApiError::Denied(e.to_string()))?;
                    Ok(json!({ "ok": true }))
                }
            }
            "close" => {
                #[cfg(not(desktop))]
                {
                    let _ = args;
                    Err(ApiError::Denied("mobile multi-tab lands with instance-swap (M2)".into()))
                }
                #[cfg(desktop)]
                {
                    let id = args.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                    manager::close_tab(&self.app, TabId(id)).map_err(|e| ApiError::Denied(e.to_string()))?;
                    Ok(json!({ "ok": true }))
                }
            }
            _ => Err(ApiError::UnknownMethod { module: self.name().to_owned(), method: method.to_owned() }),
        }
    }
}

pub struct BookmarksModule {
    db: Arc<Mutex<surf_core::storage::Db>>,
}

impl BookmarksModule {
    pub fn new(db: Arc<Mutex<surf_core::storage::Db>>) -> Self {
        Self { db }
    }
}

impl ApiModule for BookmarksModule {
    fn name(&self) -> &'static str {
        "bookmarks"
    }

    fn call(&self, _ctx: &ExtContext, method: &str, args: &Value) -> Result<Value, ApiError> {
        let db = self.db.lock().unwrap();
        match method {
            "add" => {
                let url = str_arg(args, "url")?;
                let parsed = url::Url::parse(&url).map_err(|e| ApiError::Denied(e.to_string()))?;
                let title = args.get("title").and_then(|v| v.as_str());
                db.add_bookmark(&parsed, title).map_err(|e| ApiError::Denied(e.to_string()))?;
                Ok(json!({ "ok": true }))
            }
            "remove" => {
                let url = str_arg(args, "url")?;
                let parsed = url::Url::parse(&url).map_err(|e| ApiError::Denied(e.to_string()))?;
                db.remove_bookmark(&parsed).map_err(|e| ApiError::Denied(e.to_string()))?;
                Ok(json!({ "ok": true }))
            }
            "list" => db
                .bookmarks()
                .map(|list| {
                    json!(list
                        .into_iter()
                        .map(|(u, t)| json!({ "url": u.as_str(), "title": t }))
                        .collect::<Vec<_>>())
                })
                .map_err(|e| ApiError::Denied(e.to_string())),
            _ => Err(ApiError::UnknownMethod { module: self.name().to_owned(), method: method.to_owned() }),
        }
    }
}

/// fetch(url, {method, body, headers}) with a 1 MiB response cap. The
/// request body is capped at the same size.
pub struct FetchModule;

impl ApiModule for FetchModule {
    fn name(&self) -> &'static str {
        "fetch"
    }

    fn call(&self, _ctx: &ExtContext, method: &str, args: &Value) -> Result<Value, ApiError> {
        match method {
            "fetch" => {
                let url = str_arg(args, "url")?;
                let http = url::Url::parse(&url).map_err(|e| ApiError::Denied(e.to_string()))?;
                if !matches!(http.scheme(), "http" | "https") {
                    return Err(ApiError::Denied("only http(s) fetch is supported".into()));
                }
                let verb = args.get("method").and_then(|v| v.as_str()).unwrap_or("GET").to_ascii_uppercase();
                if !matches!(verb.as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD") {
                    return Err(ApiError::Denied(format!("unsupported method: {verb}")));
                }
                let body = args.get("body").and_then(|v| v.as_str()).map_or(String::new(), str::to_owned);
                if body.len() > 1024 * 1024 {
                    return Err(ApiError::Denied("request body exceeds 1 MiB".into()));
                }
                let agent = ureq::AgentBuilder::new().build();
                let mut request = agent.request(&verb, http.as_str());
                if let Some(headers) = args.get("headers").and_then(|v| v.as_object()) {
                    for (name, value) in headers {
                        let value = value.as_str().ok_or_else(|| ApiError::Denied("header values must be strings".into()))?;
                        // Host overrides would let an extension spoof origin.
                        if name.eq_ignore_ascii_case("host") {
                            return Err(ApiError::Denied("host header cannot be overridden".into()));
                        }
                        request = request.set(name, value);
                    }
                }
                let resp = if body.is_empty() {
                    request.call().map_err(|e| ApiError::Denied(e.to_string()))?
                } else {
                    request
                        .send_bytes(body.as_bytes())
                        .map_err(|e| ApiError::Denied(e.to_string()))?
                };
                let status = resp.status();
                let mut body = String::new();
                resp.into_reader()
                    .take(1024 * 1024)
                    .read_to_string(&mut body)
                    .map_err(|e| ApiError::Denied(e.to_string()))?;
                Ok(json!({ "status": status, "body": body }))
            }
            _ => Err(ApiError::UnknownMethod { module: self.name().to_owned(), method: method.to_owned() }),
        }
    }
}

use std::io::Read;

/// Alarms module: the timer bookkeeping lives in the API registry; the
/// wakeup loop is driven by bg.rs's scheduler.
pub struct AlarmsModule {
    store: Arc<Mutex<std::collections::HashMap<(String, String), AlarmState>>>,
}

#[derive(Clone)]
pub struct AlarmState {
    pub at_ms: u64,
    pub period_ms: Option<u64>,
}

impl AlarmsModule {
    pub fn new(store: Arc<Mutex<std::collections::HashMap<(String, String), AlarmState>>>) -> Self {
        Self { store }
    }
}

impl ApiModule for AlarmsModule {
    fn name(&self) -> &'static str {
        "alarms"
    }

    fn call(&self, ctx: &ExtContext, method: &str, args: &Value) -> Result<Value, ApiError> {
        match method {
            "create" => {
                let name = str_arg(args, "name")?;
                let in_ms = args.get("inMs").and_then(|v| v.as_u64()).unwrap_or(60_000);
                let period = args.get("periodMs").and_then(|v| v.as_u64());
                self.store.lock().unwrap().insert(
                    (ctx.ext.0.clone(), name),
                    AlarmState { at_ms: now_ms() + in_ms, period_ms: period },
                );
                Ok(json!({ "ok": true }))
            }
            "clear" => {
                let name = str_arg(args, "name")?;
                self.store.lock().unwrap().remove(&(ctx.ext.0.clone(), name));
                Ok(json!({ "ok": true }))
            }
            "list" => {
                let list: Vec<Value> = self
                    .store
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|((ext, _), _)| ext == &ctx.ext.0)
                    .map(|((_, name), st)| json!({ "name": name, "atMs": st.at_ms, "periodMs": st.period_ms }))
                    .collect();
                Ok(json!(list))
            }
            _ => Err(ApiError::UnknownMethod { module: self.name().to_owned(), method: method.to_owned() }),
        }
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn build_host(app: tauri::AppHandle) -> ApiHost {
    let state = app.state::<BrowserState>();
    let db = state.db.clone();
    let alarm_store = state.alarms.clone();
    let mut host = ApiHost::new();
    host.register(Arc::new(StorageModule::new(db)));
    host.register(Arc::new(TabsModule::new(app.clone())));
    host.register(Arc::new(BookmarksModule::new(state.db.clone())));
    host.register(Arc::new(FetchModule));
    host.register(Arc::new(AlarmsModule::new(alarm_store)));
    host
}
