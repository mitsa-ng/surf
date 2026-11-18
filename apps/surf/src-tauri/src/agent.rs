//! Agent providers: local AG-UI event sources.
//!
//! The agent panel is an AG-UI native chat (`agui.rs` owns the wire
//! protocol and the run loop). When no AG-UI endpoint is configured,
//! these local providers answer instead, emitting the same event
//! shapes so the panel renders one way regardless of source:
//! - `DemoProvider`: deterministic tab summary, no network.
//! - `LlmProvider`: OpenAI-compatible chat completions (configurable
//!   base URL + model + API key; the key lives in the platform secret
//!   store). Single-shot completion wrapped in text-message events.
//!
//! Source precedence lives in `agui.rs`: AG-UI endpoint > LLM > demo.

use std::time::Duration;

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use url::Url;

use crate::state::{self, BrowserState};

/// One local agent turn: emit AG-UI events in order. `emit` forwards
/// each to the chrome webview verbatim.
pub trait AgentProvider: Send + Sync {
    fn run_turn(&self, app: &AppHandle, emit: &mut dyn FnMut(&Value));
}

/// The live tab list reduced to what providers need.
fn tabs_brief(app: &AppHandle) -> Vec<(u64, Option<String>, Option<Url>)> {
    let state = app.state::<BrowserState>();
    let brief: Vec<(u64, Option<String>, Option<Url>)> = state::lock(&state.model)
        .list()
        .iter()
        .map(|t| (t.id.0, t.title.clone(), t.url.clone()))
        .collect();
    brief
}

// ---------------------------------------------------------------------------
// Demo

/// Deterministic demo agent: the tab summary as streamed text. No
/// network, no LLM — the honest no-config answer.
pub struct DemoProvider;

impl AgentProvider for DemoProvider {
    fn run_turn(&self, app: &AppHandle, emit: &mut dyn FnMut(&Value)) {
        for event in demo_events(&tabs_brief(app)) {
            emit(&event);
        }
    }
}

/// The demo turn as AG-UI events, split into two content deltas so the
/// streaming render is visible. Pure and unit-testable.
pub fn demo_events(tabs: &[(u64, Option<String>, Option<Url>)]) -> Vec<Value> {
    let mut lines = String::new();
    if tabs.is_empty() {
        lines.push_str("目前沒有開啟中的分頁。");
    } else {
        lines.push_str(&format!("目前開啟 {} 個分頁：", tabs.len()));
        for (i, (_id, title, url)) in tabs.iter().enumerate() {
            let label = title
                .clone()
                .or_else(|| url.as_ref().and_then(|u| u.host_str().map(str::to_owned)))
                .unwrap_or_else(|| "新分頁".to_owned());
            let host = url.as_ref().and_then(|u| u.host_str()).map(|h| h.replace("www.", ""));
            let row = match host {
                Some(h) if !h.is_empty() && h != label => format!("{}. {label} · {h}", i + 1),
                _ => format!("{}. {label}", i + 1),
            };
            lines.push('\n');
            lines.push_str(&row);
        }
    }
    let note = "\n\n我是本地示範模式；在設定裡接上 AG-UI endpoint 或 LLM 後，這裡會回答你的問題。";
    let (first, rest) = lines.split_once('\n').unwrap_or((lines.as_str(), ""));
    vec![
        json!({ "type": "RUN_STARTED", "threadId": "demo", "runId": "demo" }),
        json!({ "type": "TEXT_MESSAGE_START", "messageId": "demo-msg", "role": "assistant" }),
        json!({ "type": "TEXT_MESSAGE_CONTENT", "messageId": "demo-msg", "delta": first }),
        json!({ "type": "TEXT_MESSAGE_CONTENT", "messageId": "demo-msg", "delta": format!("\n{rest}{note}") }),
        json!({ "type": "TEXT_MESSAGE_END", "messageId": "demo-msg" }),
        json!({ "type": "RUN_FINISHED" }),
    ]
}

// ---------------------------------------------------------------------------
// LLM (OpenAI-compatible, single-shot)

/// LLM provider over an OpenAI-compatible chat-completions endpoint.
/// The prompt carries the real tab list; the answer streams as text
/// message events (one content per paragraph).
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

/// The prompt for one LLM turn: live tabs plus the user's question.
fn llm_prompt(tabs: &[(u64, Option<String>, Option<Url>)], question: &str) -> String {
    let mut prompt = String::from("目前開啟的分頁：\n");
    if tabs.is_empty() {
        prompt.push_str("（沒有分頁）\n");
    }
    for (i, (title, url)) in tabs.iter().map(|(_, t, u)| (t, u)).enumerate() {
        let label = title
            .clone()
            .or_else(|| url.as_ref().and_then(|u| u.host_str().map(str::to_owned)))
            .unwrap_or_else(|| "新分頁".to_owned());
        prompt.push_str(&format!("{}. {label} — {}\n", i + 1, url.as_ref().map(|u| u.as_str()).unwrap_or("")));
    }
    prompt.push_str(&format!("\n使用者問：{question}"));
    prompt
}

/// An LLM answer as AG-UI events: one content delta per paragraph. Pure.
pub fn llm_events(answer: &str) -> Vec<Value> {
    let paragraphs: Vec<&str> = answer
        .split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .take(12)
        .collect();
    let mut events = vec![
        json!({ "type": "RUN_STARTED", "threadId": "llm", "runId": "llm" }),
        json!({ "type": "TEXT_MESSAGE_START", "messageId": "llm-msg", "role": "assistant" }),
    ];
    if paragraphs.is_empty() {
        events.push(json!({ "type": "TEXT_MESSAGE_CONTENT", "messageId": "llm-msg", "delta": "（沒有內容）" }));
    }
    for (i, p) in paragraphs.iter().enumerate() {
        let delta = if i == 0 { (*p).to_owned() } else { format!("\n\n{p}") };
        events.push(json!({ "type": "TEXT_MESSAGE_CONTENT", "messageId": "llm-msg", "delta": delta }));
    }
    events.push(json!({ "type": "TEXT_MESSAGE_END", "messageId": "llm-msg" }));
    events.push(json!({ "type": "RUN_FINISHED" }));
    events
}

/// A failed local turn, still protocol-shaped so the panel shows one
/// error card. Pure.
pub fn error_events(message: &str) -> Vec<Value> {
    vec![
        json!({ "type": "RUN_STARTED", "threadId": "local", "runId": "local" }),
        json!({ "type": "RUN_ERROR", "message": message }),
    ]
}

impl AgentProvider for LlmProvider {
    fn run_turn(&self, app: &AppHandle, emit: &mut dyn FnMut(&Value)) {
        // The question is the latest user message on the thread.
        let question = crate::agui::latest_user_message().unwrap_or_default();
        let prompt = llm_prompt(&tabs_brief(app), &question);
        let events = match self.call(&prompt) {
            Ok(answer) => llm_events(&answer),
            Err(e) => error_events(&format!("LLM 連線失敗：{e}")),
        };
        for event in events {
            emit(&event);
        }
    }
}

/// Read the LLM provider config: settings rows for base URL and model,
/// secret store for the API key. None until all three exist.
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

/// The local provider for this turn: LLM when configured, demo
/// otherwise. (AG-UI endpoints take precedence over both; see
/// `agui::agent_chat_send`.)
pub fn provider_for(app: &AppHandle) -> Box<dyn AgentProvider> {
    match llm_config(app) {
        Some(llm) => Box::new(llm),
        None => Box::new(DemoProvider),
    }
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
/// secret store keeps the secret); `clearKey` removes it.
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
    fn demo_events_stream_real_tab_labels() {
        let tabs = vec![
            (1u64, Some("Example Domain".into()), Some(url("https://example.org/"))),
            (2, None, Some(url("https://a.example.com/x"))),
            (3, None, None),
        ];
        let events = demo_events(&tabs);
        assert_eq!(events[0]["type"], "RUN_STARTED");
        assert_eq!(events[1]["type"], "TEXT_MESSAGE_START");
        assert_eq!(events[1]["messageId"], "demo-msg");
        let joined = events
            .iter()
            .filter(|e| e["type"] == "TEXT_MESSAGE_CONTENT")
            .map(|e| e["delta"].as_str().unwrap())
            .collect::<String>();
        assert!(joined.contains("目前開啟 3 個分頁"));
        assert!(joined.contains("Example Domain · example.org"));
        assert!(joined.contains("a.example.com"));
        assert!(joined.contains("新分頁"));
        assert_eq!(events.last().unwrap()["type"], "RUN_FINISHED");
        // Every text event names the same message; the stream opens
        // before content and closes after it.
        let types: Vec<&str> = events.iter().map(|e| e["type"].as_str().unwrap()).collect();
        assert_eq!(types.first(), Some(&"RUN_STARTED"));
        assert_eq!(types.last(), Some(&"RUN_FINISHED"));
        assert!(types.windows(2).any(|w| w == ["TEXT_MESSAGE_START", "TEXT_MESSAGE_CONTENT"]));
        assert!(types.windows(2).any(|w| w == ["TEXT_MESSAGE_CONTENT", "TEXT_MESSAGE_END"]));
    }

    #[test]
    fn demo_events_empty_tabs_stay_honest() {
        let events = demo_events(&[]);
        let joined = events
            .iter()
            .filter(|e| e["type"] == "TEXT_MESSAGE_CONTENT")
            .map(|e| e["delta"].as_str().unwrap())
            .collect::<String>();
        assert!(joined.contains("目前沒有開啟中的分頁。"));
        assert!(!joined.contains("example"));
    }

    #[test]
    fn llm_answers_stream_as_paragraph_deltas() {
        let events = llm_events("第一段。\n\n第二段。");
        let types: Vec<&str> = events.iter().map(|e| e["type"].as_str().unwrap()).collect();
        assert_eq!(types.first(), Some(&"RUN_STARTED"));
        assert_eq!(types.last(), Some(&"RUN_FINISHED"));
        let deltas: Vec<&str> = events
            .iter()
            .filter(|e| e["type"] == "TEXT_MESSAGE_CONTENT")
            .map(|e| e["delta"].as_str().unwrap())
            .collect();
        assert_eq!(deltas, vec!["第一段。", "\n\n第二段。"]);
        // Empty answers stay honest instead of streaming nothing.
        let empty = llm_events("   ");
        assert!(empty.iter().any(|e| e["delta"] == "（沒有內容）"));
    }

    #[test]
    fn error_events_are_protocol_shaped() {
        let events = error_events("連線失敗");
        assert_eq!(events[0]["type"], "RUN_STARTED");
        assert_eq!(events[1]["type"], "RUN_ERROR");
        assert_eq!(events[1]["message"], "連線失敗");
    }

    #[test]
    fn completion_parses_openai_shape() {
        let body = r#"{"choices":[{"message":{"role":"assistant","content":"你好"}}]}"#;
        assert_eq!(parse_completion(body).as_deref(), Some("你好"));
        assert_eq!(parse_completion(r#"{"choices":[]}"#), None);
        assert_eq!(parse_completion("not json"), None);
    }

    #[test]
    fn llm_endpoint_and_prompt_shape() {
        let llm = LlmProvider {
            base_url: "https://api.example.com/v1/".into(),
            model: "gpt-test".into(),
            api_key: "k".into(),
        };
        assert_eq!(llm.endpoint(), "https://api.example.com/v1/chat/completions");
        let body = llm.request_body("hello");
        assert_eq!(body["model"], "gpt-test");
        assert_eq!(body["messages"][1]["content"], "hello");
        let tabs = vec![(1u64, Some("T".into()), Some(url("https://e.dev/")))];
        let prompt = llm_prompt(&tabs, "現在幾個分頁？");
        assert!(prompt.contains("1. T — https://e.dev/"));
        assert!(prompt.contains("使用者問：現在幾個分頁？"));
    }
}
