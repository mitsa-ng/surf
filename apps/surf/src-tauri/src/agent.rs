//! Agent-to-User Interface (a2ui) provider pipeline.
//!
//! The chrome renders agent UIs with the a2ui web components; this module
//! is the Rust side of the stream. An `AgentProvider` turns browser state
//! into a2ui v0.9 protocol messages and pushes them to the chrome webview
//! as `agent://message` events.
//!
//! Two providers ship:
//! - `DemoProvider`: deterministic, reads the live tab model, and its
//!   tab rows are real a2ui Buttons wired to `agent_action` (click a tab
//!   in the panel to activate it). No LLM involved.
//! - `LlmProvider`: OpenAI-compatible chat completions (configurable
//!   base URL + model + API key; the key lives in the macOS keychain).
//!   The answer text is rendered as a2ui Text components. Single-shot
//!   completion, no SSE yet.
//!
//! Interactive components dispatch through a2ui's `surface.onAction`
//! (`action.event.name` + `context`), which the chrome forwards to the
//! `agent_action` command below.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use url::Url;

use crate::state::{self, BrowserState};

pub const MESSAGE_EVENT: &str = "agent://message";
pub const SURFACE_ID: &str = "surf-agent";
/// Matches @a2ui/lit v0_9 basicCatalog.id; the catalog the renderer
/// registers, so createSurface must name exactly this.
pub const BASIC_CATALOG_ID: &str = "https://a2ui.org/specification/v0_9/catalogs/basic/catalog.json";

/// One agent turn: an ordered stream of a2ui messages for the panel.
pub trait AgentProvider: Send + Sync {
    fn plan(&self, app: &AppHandle) -> Vec<Value>;
}

/// Deterministic demo agent: reads the live tab model and streams a card
/// of the real open tabs. Each row is a Button whose action activates the
/// tab through `agent_action`.
pub struct DemoProvider;

impl AgentProvider for DemoProvider {
    fn plan(&self, app: &AppHandle) -> Vec<Value> {
        let state = app.state::<BrowserState>();
        let tabs = state::lock(&state.model).list().to_vec();
        demo_messages(&tabs.iter().map(|t| (t.id.0, t.title.clone(), t.url.clone())).collect::<Vec<_>>())
    }
}

/// The message plan for a tab-summary turn. Pure so the shape and the
/// streaming cadence are unit-testable without a window.
fn demo_messages(tabs: &[(u64, Option<String>, Option<Url>)]) -> Vec<Value> {
    let title = format!("目前開啟的分頁（{}）", tabs.len());
    let mut components = vec![
        json!({ "id": "root", "component": "Column", "children": ["card"], "align": "stretch" }),
        json!({ "id": "card", "component": "Card", "child": "inner" }),
        json!({ "id": "inner", "component": "Column", "children": ["heading", "list", "note"] }),
        json!({ "id": "heading", "component": "Text", "text": { "path": "/title" }, "variant": "h4" }),
    ];
    if tabs.is_empty() {
        components.push(json!({ "id": "list", "component": "Text", "text": "沒有開啟中的分頁。" }));
    } else {
        let ids: Vec<String> = (0..tabs.len()).map(|i| format!("row{i}")).collect();
        for (i, (tab_id, t, u)) in tabs.iter().enumerate() {
            let label = t.clone()
                .or_else(|| u.as_ref().map(|url| url.host_str().unwrap_or_default().to_owned()))
                .unwrap_or_else(|| "新分頁".to_owned());
            let host = u
                .as_ref()
                .and_then(|url| url.host_str())
                .map(|h| h.replace("www.", ""))
                .unwrap_or_default();
            let text = if host.is_empty() || label == host {
                label
            } else {
                format!("{label} · {host}")
            };
            // The action loop: clicking the row activates the tab. The
            // context carries the tab id literally (a2ui guidance: no
            // paths for static ids).
            components.push(json!({
                "id": format!("row{i}"),
                "component": "Button",
                "child": format!("lbl{i}"),
                "variant": "borderless",
                "action": { "event": { "name": "activate_tab", "context": { "tabId": tab_id } } },
            }));
            components.push(json!({ "id": format!("lbl{i}"), "component": "Text", "text": text }));
        }
        components.push(json!({ "id": "list", "component": "List", "children": ids, "direction": "vertical" }));
    }
    components.push(json!({ "id": "note", "component": "Text", "text": "點一下分頁即可切換過去。LLM 連線後，這裡會回答你的問題。", "variant": "caption" }));

    vec![
        json!({
            "version": "v0.9",
            "createSurface": {
                "surfaceId": SURFACE_ID,
                "catalogId": BASIC_CATALOG_ID,
                "theme": { "primaryColor": "#0a7fa8", "agentDisplayName": "Surf Agent" },
            },
        }),
        json!({
            "version": "v0.9",
            "updateComponents": { "surfaceId": SURFACE_ID, "components": components },
        }),
        json!({
            "version": "v0.9",
            "updateDataModel": { "surfaceId": SURFACE_ID, "path": "/", "value": { "title": title } },
        }),
    ]
}

/// LLM provider over an OpenAI-compatible chat-completions endpoint. The
/// prompt carries the real tab list; the answer renders as Text rows.
pub struct LlmProvider {
    pub base_url: String,
    pub model: String,
    pub api_key: String,
}

impl LlmProvider {
    fn endpoint(&self) -> String {
        let base = self.base_url.trim_end_matches('/');
        format!("{base}/chat/completions")
    }

    fn request_body(&self, prompt: &str) -> Value {
        json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": "你是 Surf 瀏覽器的助手。用繁體中文簡短回答，語氣安靜直接，不誇飾。" },
                { "role": "user", "content": prompt }
            ]
        })
    }

    fn call(&self, prompt: &str) -> Result<String, String> {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(6))
            .timeout(Duration::from_secs(60))
            .build();
        let resp = agent
            .post(&self.endpoint())
            .set("Authorization", &format!("Bearer {}", self.api_key))
            .set("Content-Type", "application/json")
            .send_bytes(serde_json::to_vec(&self.request_body(prompt)).map_err(|e| e.to_string())?.as_slice())
            .map_err(|e| format!("LLM 請求失敗：{e}"))?;
        let status = resp.status();
        let mut body = String::new();
        use std::io::Read;
        resp.into_reader()
            .take(1024 * 1024)
            .read_to_string(&mut body)
            .map_err(|e| e.to_string())?;
        if status != 200 {
            return Err(format!("LLM 回應 {status}：{}", body.chars().take(200).collect::<String>()));
        }
        parse_completion(&body).ok_or_else(|| "LLM 回應不是預期的 JSON 形狀".to_owned())
    }
}

/// Pull `choices[0].message.content` out of an OpenAI-style response.
fn parse_completion(body: &str) -> Option<String> {
    let v: Value = serde_json::from_str(body).ok()?;
    v.get("choices")?
        .get(0)?
        .get("message")?
        .get("content")?
        .as_str()
        .map(str::to_owned)
}

impl AgentProvider for LlmProvider {
    fn plan(&self, app: &AppHandle) -> Vec<Value> {
        let state = app.state::<BrowserState>();
        let tabs = state::lock(&state.model).list().to_vec();
        let mut prompt = String::from("目前開啟的分頁：\n");
        if tabs.is_empty() {
            prompt.push_str("（沒有分頁）\n");
        }
        for (i, t) in tabs.iter().enumerate() {
            let label = t.title.clone()
                .or_else(|| t.url.as_ref().and_then(|u| u.host_str().map(str::to_owned)))
                .unwrap_or_else(|| "新分頁".to_owned());
            prompt.push_str(&format!("{}. {label} — {}\n", i + 1, t.url.as_ref().map(|u| u.as_str()).unwrap_or("")));
        }
        prompt.push_str("\n請摘要目前狀態，或告訴我下一步可以做什麼（三句內）。");
        match self.call(&prompt) {
            Ok(answer) => llm_messages(&answer),
            Err(e) => llm_messages(&format!("LLM 連線失敗：{e}")),
        }
    }
}

/// Wrap an LLM answer into a2ui messages: paragraphs become Text rows in
/// one card. Pure and unit-testable.
fn llm_messages(answer: &str) -> Vec<Value> {
    let paragraphs: Vec<&str> = answer
        .split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .take(12)
        .collect();
    let mut components = vec![
        json!({ "id": "root", "component": "Column", "children": ["card"], "align": "stretch" }),
        json!({ "id": "card", "component": "Card", "child": "inner" }),
    ];
    let mut ids: Vec<String> = (0..paragraphs.len()).map(|i| format!("p{i}")).collect();
    for (i, p) in paragraphs.iter().enumerate() {
        components.push(json!({ "id": format!("p{i}"), "component": "Text", "text": p }));
    }
    if paragraphs.is_empty() {
        components.push(json!({ "id": "p0", "component": "Text", "text": "（沒有內容）" }));
        ids.push("p0".to_owned());
    }
    components.push(json!({ "id": "inner", "component": "Column", "children": ids, "align": "stretch" }));
    vec![
        json!({
            "version": "v0.9",
            "createSurface": {
                "surfaceId": SURFACE_ID,
                "catalogId": BASIC_CATALOG_ID,
                "theme": { "primaryColor": "#0a7fa8", "agentDisplayName": "Surf Agent" },
            },
        }),
        json!({
            "version": "v0.9",
            "updateComponents": { "surfaceId": SURFACE_ID, "components": components },
        }),
    ]
}

/// Streaming cadence for a plan: how long to wait before each message.
/// The pause simulates an agent thinking so the panel visibly renders
/// progressively, which is the point of the a2ui pipeline.
fn tick_for(index: usize) -> Duration {
    match index {
        0 => Duration::from_millis(150),
        _ => Duration::from_millis(420),
    }
}

static STREAM_RUNNING: AtomicBool = AtomicBool::new(false);

/// Read the LLM provider config: settings rows for base URL and model,
/// keychain for the API key. None until all three exist.
pub fn llm_config(app: &AppHandle) -> Option<LlmProvider> {
    let (base_url, model) = {
        let state = app.state::<BrowserState>();
        let db = state::lock(&state.db);
        let base = db.ext_kv_get("sys", "setting:agent_llm_base_url").ok().flatten()?;
        let model = db.ext_kv_get("sys", "setting:agent_llm_model").ok().flatten()?;
        (base, model)
    };
    if base_url.trim().is_empty() || model.trim().is_empty() {
        return None;
    }
    let api_key = {
        let state = app.state::<BrowserState>();
        state.secrets.get("agent_llm_api_key").ok()?
    };
    if api_key.trim().is_empty() {
        return None;
    }
    Some(LlmProvider { base_url, model, api_key })
}

/// Build the provider for this open: LLM when configured, demo otherwise.
fn provider_for(app: &AppHandle) -> Box<dyn AgentProvider> {
    match llm_config(app) {
        Some(llm) => Box::new(llm),
        None => Box::new(DemoProvider),
    }
}

fn start_stream(app: &AppHandle) {
    if STREAM_RUNNING.swap(true, Ordering::SeqCst) {
        return; // A stream is already feeding the open panel.
    }
    let handle = Arc::new(app.clone());
    let provider = provider_for(app);
    std::thread::spawn(move || {
        let plan = provider.plan(&handle);
        for (i, message) in plan.into_iter().enumerate() {
            if !STREAM_RUNNING.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(tick_for(i));
            if !STREAM_RUNNING.load(Ordering::SeqCst) {
                break;
            }
            let _ = handle.emit_to(crate::layout::CHROME_LABEL, MESSAGE_EVENT, message);
        }
        STREAM_RUNNING.store(false, Ordering::SeqCst);
    });
}

/// Open or close the agent panel. Opening starts one provider stream for
/// the current state; closing aborts it. The chrome rebuilds its surface
/// from scratch each open, so a stale stream can never leak into a fresh
/// panel.
#[tauri::command]
pub fn agent_panel_set_open(app: AppHandle, open: bool) -> Result<(), String> {
    if !open {
        STREAM_RUNNING.store(false, Ordering::SeqCst);
        return Ok(());
    }
    start_stream(&app);
    Ok(())
}

/// A component action arrived from the a2ui surface (Button clicks and
/// friends). Routed here by name; after acting, the panel re-plans so it
/// reflects the new state.
#[tauri::command]
pub fn agent_action(app: AppHandle, name: String, context: serde_json::Value) -> Result<(), String> {
    let outcome = match name.as_str() {
        "activate_tab" => {
            let tab_id = context.get("tabId").and_then(|v| v.as_u64()).unwrap_or(0);
            #[cfg(desktop)]
            {
                crate::manager::activate_tab(&app, surf_core::tabs::TabId(tab_id)).map_err(|e| e.to_string())?;
            }
            #[cfg(mobile)]
            {
                crate::manager::activate_tab_mobile(&app, surf_core::tabs::TabId(tab_id))
                    .map_err(|e| e.to_string())?;
            }
            format!("已切換到分頁 {tab_id}")
        }
        other => format!("未知的 agent 動作：{other}"),
    };
    let _ = outcome;
    // The demo card shows live tab state; re-stream it after the action.
    start_stream(&app);
    Ok(())
}

/// Read the agent LLM config for the settings panel. The API key is never
/// returned; `hasKey` says whether one is stored.
#[tauri::command]
pub fn agent_llm_config_get(app: AppHandle) -> serde_json::Value {
    let state = app.state::<BrowserState>();
    let db = state::lock(&state.db);
    let base = db.ext_kv_get("sys", "setting:agent_llm_base_url").ok().flatten().unwrap_or_default();
    let model = db.ext_kv_get("sys", "setting:agent_llm_model").ok().flatten().unwrap_or_default();
    drop(db);
    let has_key = state.secrets.get("agent_llm_api_key").map(|k| !k.trim().is_empty()).unwrap_or(false);
    json!({ "baseUrl": base, "model": model, "hasKey": has_key })
}

/// Save the agent LLM config. An empty key keeps the stored one (the
/// keychain keeps the secret); `clearKey` removes it.
#[tauri::command]
pub fn agent_llm_config_set(
    app: AppHandle,
    base_url: String,
    model: String,
    api_key: Option<String>,
    clear_key: bool,
) -> Result<(), String> {
    {
        let state = app.state::<BrowserState>();
        let db = state::lock(&state.db);
        let _ = db.ext_kv_set("sys", "setting:agent_llm_base_url", base_url.trim());
        let _ = db.ext_kv_set("sys", "setting:agent_llm_model", model.trim());
    }
    if clear_key {
        let state = app.state::<BrowserState>();
        let _ = state.secrets.delete("agent_llm_api_key");
    } else if let Some(key) = api_key.map(|k| k.trim().to_owned()).filter(|k| !k.is_empty()) {
        let state = app.state::<BrowserState>();
        state.secrets.put("agent_llm_api_key", &key).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn plan_shapes_valid_v0_9_messages_with_button_actions() {
        let tabs = vec![
            (1u64, Some("Example Domain".into()), Some(url("https://example.org/"))),
            (2, None, Some(url("https://a.example.com/x"))),
            (3, None, None),
        ];
        let plan = demo_messages(&tabs);
        assert_eq!(plan.len(), 3);
        assert_eq!(plan[0]["version"], "v0.9");
        assert_eq!(plan[0]["createSurface"]["surfaceId"], SURFACE_ID);
        assert_eq!(plan[0]["createSurface"]["catalogId"], BASIC_CATALOG_ID);
        assert_eq!(plan[0]["createSurface"]["theme"]["primaryColor"], "#0a7fa8");

        let components = &plan[1]["updateComponents"]["components"];
        let root = components.as_array().unwrap().iter().find(|c| c["id"] == "root").unwrap();
        assert_eq!(root["component"], "Column");
        // Exactly one root, every child reference resolves.
        let ids: Vec<&str> = components.as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap()).collect();
        assert_eq!(ids.iter().filter(|i| **i == "root").count(), 1);
        let referenced: Vec<String> = ["card", "heading", "list", "note"]
            .iter()
            .copied()
            .chain(
                components.as_array().unwrap().iter().flat_map(|c| -> Vec<&str> {
                    let mut out: Vec<&str> = c["children"].as_array().map(|a| a.iter().map(|v| v.as_str().unwrap()).collect()).unwrap_or_default();
                    if let Some(child) = c["child"].as_str() {
                        out.push(child);
                    }
                    out
                }),
            )
            .map(|s| s.to_string())
            .collect();
        for r in &referenced {
            assert!(ids.contains(&r.as_str()), "unresolved child reference {r}");
        }
        // One button per tab, each carrying the activate_tab action with
        // the real tab id.
        let buttons: Vec<&Value> = components.as_array().unwrap().iter().filter(|c| c["component"] == "Button").collect();
        assert_eq!(buttons.len(), 3);
        assert_eq!(buttons[0]["action"]["event"]["name"], "activate_tab");
        assert_eq!(buttons[0]["action"]["event"]["context"]["tabId"], 1);
        assert_eq!(buttons[2]["action"]["event"]["context"]["tabId"], 3);
        assert_eq!(buttons[0]["variant"], "borderless");
        // Real labels only.
        let joined = components.to_string();
        assert!(joined.contains("Example Domain · example.org"));
        assert!(joined.contains("a.example.com"));
        assert!(joined.contains("新分頁"));

        assert_eq!(plan[2]["updateDataModel"]["value"]["title"], "目前開啟的分頁（3）");
    }

    #[test]
    fn empty_tabs_plan_stays_honest() {
        let plan = demo_messages(&[]);
        let joined = plan[1]["updateComponents"]["components"].to_string();
        assert!(joined.contains("沒有開啟中的分頁。"));
        assert!(!joined.contains("example"));
        assert!(!joined.contains("Button"), "no dead controls without tabs");
    }

    #[test]
    fn llm_answers_wrap_into_paragraph_rows() {
        let plan = llm_messages("第一段。\n\n第二段。");
        assert_eq!(plan.len(), 2);
        let components = plan[1]["updateComponents"]["components"].as_array().unwrap();
        let texts: Vec<&Value> = components.iter().filter(|c| c["component"] == "Text").collect();
        assert_eq!(texts.len(), 2);
        assert_eq!(texts[0]["text"], "第一段。");
        assert_eq!(texts[1]["text"], "第二段。");
        // Empty answers stay honest instead of rendering nothing.
        let empty = llm_messages("   ");
        let comps = empty[1]["updateComponents"]["components"].as_array().unwrap();
        assert!(comps.iter().any(|c| c["text"] == "（沒有內容）"));
    }

    #[test]
    fn completion_parses_openai_shape() {
        let body = r#"{"choices":[{"message":{"role":"assistant","content":"你好"}}]}"#;
        assert_eq!(parse_completion(body).as_deref(), Some("你好"));
        assert_eq!(parse_completion(r#"{"choices":[]}"#), None);
        assert_eq!(parse_completion("not json"), None);
    }

    #[test]
    fn llm_endpoint_and_body_shape() {
        let llm = LlmProvider {
            base_url: "https://api.example.com/v1/".into(),
            model: "gpt-test".into(),
            api_key: "k".into(),
        };
        assert_eq!(llm.endpoint(), "https://api.example.com/v1/chat/completions");
        let body = llm.request_body("hello");
        assert_eq!(body["model"], "gpt-test");
        assert_eq!(body["messages"][1]["content"], "hello");
    }

    #[test]
    fn ticks_space_the_stream() {
        assert!(tick_for(0) < tick_for(1));
    }
}
