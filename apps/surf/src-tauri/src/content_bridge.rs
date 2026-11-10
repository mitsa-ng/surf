//! Token-bound content-script bridge (layer-3 boundary).
//!
//! Content webviews load remote pages, so they get no Tauri capability.
//! The host injects one init script carrying a per-tab token; page-side
//! scripts call back through two commands (`cs_bootstrap`, `cs_send`)
//! that require the token. The token is readable by page JS — a page can
//! forge messages *for its own tab*. Real isolated-world isolation needs
//! platform APIs wry does not expose; documented in NOTES.md.

use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::Value;
use surf_core::tabs::TabId;
use surf_ext_host::router::{Envelope, Endpoint, MessageBody};
use tauri::{AppHandle, Manager};

use crate::state::{self, BrowserState};
use crate::tabview::TabView;

pub struct TokenRegistry {
    tokens: Mutex<HashMap<String, TabId>>,
}

impl Default for TokenRegistry {
    fn default() -> Self {
        Self { tokens: Mutex::new(HashMap::new()) }
    }
}

impl TokenRegistry {
    pub fn issue(&self, id: TabId) -> String {
        let token = random_token();
        self.tokens.lock().unwrap().insert(token.clone(), id);
        token
    }

    pub fn revoke(&self, id: TabId) {
        self.tokens.lock().unwrap().retain(|_, v| *v != id);
    }

    pub fn resolve(&self, token: &str) -> Option<TabId> {
        self.tokens.lock().unwrap().get(token).copied()
    }
}

fn random_token() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..32).map(|_| format!("{:x}", rng.gen::<u8>() & 0xf)).collect()
}

/// Injected at webview creation; survives every navigation. It pulls the
/// matching scripts per navigation from the host, so content scripts are
/// re-planned on each page without recreating the webview.
pub fn bootstrap_script(token: &str) -> String {
    format!(
        r#"(() => {{
  const TOKEN = {token:?};
  const send = (name, payload, ext) => window.__TAURI_INTERNALS__
    ? window.__TAURI_INTERNALS__.invoke("cs_send", {{ token: TOKEN, name, payload: payload === undefined ? null : payload, ext: ext || null }})
    : null;
  window.__surf = {{ token: TOKEN, send, receive: null }};
  const evalAll = (scripts) => {{
    for (const s of scripts || []) {{
      try {{ (0, eval)(s.code); }} catch (e) {{ console.error("surf content script failed", e); }}
    }}
  }};
  const run = () => {{
    window.__TAURI_INTERNALS__.invoke("cs_bootstrap", {{ token: TOKEN, url: location.href }})
      .then((res) => {{
        document.documentElement.dataset.surfBridge = "ready";
        const idle = (res.scripts || []).filter((s) => s.runAt === "documentidle" || s.runAt === "documentend");
        const now = (res.scripts || []).filter((s) => s.runAt !== "documentidle" && s.runAt !== "documentend");
        evalAll(now);
        if (idle.length) {{
          const runIdle = () => evalAll(idle);
          if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", runIdle, {{ once: true }});
          else runIdle();
        }}
      }})
      .catch((e) => {{
        document.documentElement.dataset.surfBridge = "error:" + e;
        console.error("surf bootstrap failed", e);
      }});
  }};
  // Tauri's internals script may land after ours at document start; poll.
  let tries = 0;
  const wait = () => {{
    if (window.__TAURI_INTERNALS__) {{ run(); return; }}
    if (++tries > 100) {{ document.documentElement.dataset.surfBridge = "no-internals"; return; }}
    setTimeout(wait, 50);
  }};
  wait();
  window.__surfReceive = (name, payload) => {{
    if (typeof window.__surfOnEvent === "function") window.__surfOnEvent(name, payload);
  }};
}})();"#,
        token = token
    )
}

/// Host → content webview delivery.
pub fn send_to_tab(app: &AppHandle, id: TabId, name: &str, payload: &Value) {
    if let Some(view) = state::content_webview(app, id) {
        let json = serde_json::json!({ "name": name, "payload": payload });
        let payload_str = json.get("payload").map(|p| p.to_string()).unwrap_or_else(|| "null".into());
        let js = format!("window.__surfReceive && window.__surfReceive({:?}, {})", name, payload_str);
        let _ = view.eval(&js);
    }
}

pub fn register_content_sink(app: &AppHandle, id: TabId) {
    let state = app.state::<BrowserState>();
    let handle = app.clone();
    let sink: std::sync::Arc<dyn surf_ext_host::router::MessageSink> = std::sync::Arc::new(ContentSink { tab: id, app: handle });
    let mut router = state::lock(&state.router);
    router.subscribe(Endpoint::Content(id), sink);
}

struct ContentSink {
    tab: TabId,
    app: AppHandle,
}

impl surf_ext_host::router::MessageSink for ContentSink {
    fn deliver(&self, envelope: Envelope) -> Result<(), surf_ext_host::router::RouterError> {
        if let MessageBody::Event { name, payload } = &envelope.body {
            send_to_tab(&self.app, self.tab, name, payload);
            Ok(())
        } else {
            Err(surf_ext_host::router::RouterError::Rejected("content sink accepts events only".into()))
        }
    }
}

