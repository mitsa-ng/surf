//! AG-UI protocol client (https://github.com/ag-ui-protocol/ag-ui).
//!
//! The agent panel is an AG-UI native chat: Rust owns the run loop
//! (POST RunAgentInput → SSE event stream), executes browser tools,
//! and forwards every event verbatim to the chrome webview as
//! `agui://event`. The panel renders from those events alone, so all
//! three sources — AG-UI endpoint > OpenAI-compatible LLM > local
//! demo (`agent.rs`) — stream through one shape.
//!
//! Browser tools follow the protocol's client-tool shape: the tools
//! are declared in RunAgentInput, executed locally on TOOL_CALL_END,
//! and their results ride the *next* run's messages (capped at
//! MAX_FOLLOW_UPS per user turn so a looping agent cannot spin
//! forever).

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use url::Url;

use crate::state::{self, BrowserState};
use surf_core::tabs::TabId;

/// AG-UI events reach the chrome verbatim; the panel is the only
/// consumer and renders purely from this stream.
pub const EVENT: &str = "agui://event";

/// Tool-result rounds per user turn before the loop stops honestly.
const MAX_FOLLOW_UPS: usize = 5;
/// Wall-clock budget for one whole user turn, all runs combined.
const TURN_TIMEOUT: Duration = Duration::from_secs(120);
/// Per-read budget; a server silent for this long counts as one stall.
const READ_TIMEOUT: Duration = Duration::from_secs(30);
/// Consecutive read stalls (timeouts with no bytes) before giving up.
const MAX_READ_STALLS: u32 = 4;

static RUN_ACTIVE: AtomicBool = AtomicBool::new(false);
/// Conversation history (RunAgentInput messages) for the app session;
/// the panel's reset button clears it. In-memory by design (NOTES).
static THREAD: Mutex<Vec<Value>> = Mutex::new(Vec::new());
static MSG_SEQ: AtomicU64 = AtomicU64::new(0);

fn next_id(prefix: &str) -> String {
    format!("{prefix}-{}", MSG_SEQ.fetch_add(1, Ordering::Relaxed))
}

fn thread() -> std::sync::MutexGuard<'static, Vec<Value>> {
    THREAD.lock().unwrap_or_else(|p| p.into_inner())
}

fn emit(handle: &AppHandle, event: &Value) {
    let _ = handle.emit_to(crate::layout::CHROME_LABEL, EVENT, event);
}

/// The newest user question on the thread, for local providers whose
/// prompt needs the actual question rather than a summary cue.
pub fn latest_user_message() -> Option<String> {
    thread()
        .iter()
        .rev()
        .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
        .and_then(|m| m.get("content").and_then(|c| c.as_str()).map(str::to_owned))
}

// ---------------------------------------------------------------------------
// SSE wire format

/// Incremental SSE parser: `data:` lines accumulate until a blank line
/// dispatches them as one joined payload; `event:`/`id:`/`retry:` and
/// `:`-comment lines carry nothing AG-UI needs (the event type rides
/// inside the JSON). Pure so the wire format is unit-testable.
#[derive(Default)]
pub struct SseParser {
    data: Vec<String>,
}

impl SseParser {
    /// Feed one line as `BufRead::read_line` delivers it — trailing
    /// `\n` (and `\r\n`) included — so the blank-line dispatch sees an
    /// empty string. Returns the frame's joined data payload when a
    /// blank line completes it.
    pub fn feed_line(&mut self, line: &str) -> Option<String> {
        let line = line.strip_suffix('\n').unwrap_or(line);
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            let out = if self.data.is_empty() { None } else { Some(self.data.join("\n")) };
            self.data.clear();
            return out;
        }
        if let Some(rest) = line.strip_prefix("data:") {
            self.data.push(rest.trim_start_matches(' ').to_owned());
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Events

/// The event kinds this client acts on; anything else forwards verbatim
/// but drives no state.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum EventKind {
    RunStarted,
    RunFinished,
    RunError,
    TextStart,
    TextContent,
    TextEnd,
    ToolStart,
    ToolArgs,
    ToolEnd,
    Other,
}

pub fn event_kind(raw: &Value) -> EventKind {
    match raw.get("type").and_then(|t| t.as_str()).unwrap_or_default() {
        "RUN_STARTED" => EventKind::RunStarted,
        "RUN_FINISHED" => EventKind::RunFinished,
        "RUN_ERROR" => EventKind::RunError,
        "TEXT_MESSAGE_START" => EventKind::TextStart,
        "TEXT_MESSAGE_CONTENT" => EventKind::TextContent,
        "TEXT_MESSAGE_END" => EventKind::TextEnd,
        "TOOL_CALL_START" => EventKind::ToolStart,
        "TOOL_CALL_ARGS" => EventKind::ToolArgs,
        "TOOL_CALL_END" => EventKind::ToolEnd,
        _ => EventKind::Other,
    }
}

/// Accumulates one server run into thread-ready pieces. Pure: feed it
/// classified events, receive completed assistant messages and tool
/// calls as they close.
#[derive(Default)]
pub struct TurnCollector {
    texts: HashMap<String, String>,
    tools: HashMap<String, ToolCall>,
}

/// A tool call whose args stream has closed.
#[derive(Debug, PartialEq, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args_raw: String,
}

/// What a fed event completed, if anything.
#[derive(Debug, PartialEq)]
pub enum Completed {
    /// TEXT_MESSAGE_END closed a message with content.
    AssistantText { id: String, content: String },
    /// TOOL_CALL_END closed a call; `args_raw` parses with `parse_args`.
    ToolCall(ToolCall),
}

impl TurnCollector {
    pub fn feed(&mut self, kind: EventKind, raw: &Value) -> Option<Completed> {
        match kind {
            EventKind::TextContent => {
                let id = raw.get("messageId").and_then(|v| v.as_str()).unwrap_or_default();
                if let Some(delta) = raw.get("delta").and_then(|v| v.as_str()) {
                    self.texts.entry(id.to_owned()).or_default().push_str(delta);
                }
                None
            }
            EventKind::TextEnd => {
                let id = raw.get("messageId").and_then(|v| v.as_str()).unwrap_or_default();
                self.texts.remove(id).map(|content| Completed::AssistantText {
                    id: id.to_owned(),
                    content,
                })
            }
            EventKind::ToolStart => {
                // A missing id must not alias the empty-string key.
                let id = raw.get("toolCallId").and_then(|v| v.as_str()).unwrap_or("unknown");
                let id = if id.is_empty() { "unknown" } else { id };
                let name = raw.get("toolCallName").and_then(|v| v.as_str()).unwrap_or_default().to_owned();
                self.tools.insert(id.to_owned(), ToolCall { id: id.to_owned(), name, args_raw: String::new() });
                None
            }
            EventKind::ToolArgs => {
                let id = raw.get("toolCallId").and_then(|v| v.as_str()).unwrap_or_default();
                if let Some(call) = self.tools.get_mut(id) {
                    if let Some(delta) = raw.get("delta").and_then(|v| v.as_str()) {
                        call.args_raw.push_str(delta);
                    }
                }
                None
            }
            EventKind::ToolEnd => {
                let id = raw.get("toolCallId").and_then(|v| v.as_str()).unwrap_or_default();
                self.tools.remove(id).map(Completed::ToolCall)
            }
            _ => None,
        }
    }
}

/// Tool args arrive as streamed JSON fragments; empty means no args.
pub fn parse_args(raw: &str) -> Value {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return json!({});
    }
    serde_json::from_str(trimmed).unwrap_or_else(|_| json!({ "unparsed": trimmed }))
}

// ---------------------------------------------------------------------------
// RunAgentInput

/// The system prompt rides as the first message of every run; the
/// thread holds only the real conversation.
fn system_message() -> Value {
    json!({
        "id": "sys",
        "role": "system",
        "content": "你是 Surf 瀏覽器內建的助手。用繁體中文回答，語氣安靜直接，不誇飾。你可以用工具操作瀏覽器：activate_tab 切換分頁、open_url 開網址、get_tabs 列分頁；state 欄位有目前分頁快照。",
    })
}

/// Browser tools declared to the agent (RunAgentInput.tools).
pub fn browser_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "activate_tab",
            "description": "切換到指定的分頁（tabId 見 state 的分頁清單）。",
            "parameters": {
                "type": "object",
                "properties": { "tabId": { "type": "integer", "description": "目標分頁 id" } },
                "required": ["tabId"],
            },
        }),
        json!({
            "name": "open_url",
            "description": "開啟網址。newTab 為 true 開新分頁（預設），否則在目前分頁導航。",
            "parameters": {
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "要開的網址" },
                    "newTab": { "type": "boolean" },
                },
                "required": ["url"],
            },
        }),
        json!({
            "name": "get_tabs",
            "description": "列出目前開啟的分頁（id、標題、網址、作用中）。",
            "parameters": { "type": "object", "properties": {} },
        }),
    ]
}

/// Build one RunAgentInput. Pure.
pub fn run_input(thread_id: &str, thread: &[Value], tools: &[Value], state: Value) -> Value {
    let mut messages = vec![system_message()];
    messages.extend_from_slice(thread);
    json!({
        "threadId": thread_id,
        "messages": messages,
        "tools": tools,
        "state": state,
        "context": {},
    })
}

// ---------------------------------------------------------------------------
// Browser tools

/// The live tab list, shaped for agents (also the run's `state`).
pub fn tabs_value(app: &AppHandle) -> Value {
    let state = app.state::<BrowserState>();
    let model = state::lock(&state.model);
    let active = model.active_id().map(|id| id.0).unwrap_or(0);
    json!(
        model
            .list()
            .iter()
            .map(|t| {
                json!({
                    "id": t.id.0,
                    "title": t.title.clone().unwrap_or_default(),
                    "url": t.url.as_ref().map(|u| u.as_str()).unwrap_or(""),
                    "active": t.id.0 == active,
                })
            })
            .collect::<Vec<_>>()
    )
}

/// Accepts what an agent plausibly writes ("example.com/x") and still
/// refuses garbage; the omnibox rules stay a user-surface concern.
fn normalize_url(raw: &str) -> Result<Url, String> {
    let trimmed = raw.trim();
    let candidate = if trimmed.contains("://") { trimmed.to_owned() } else { format!("https://{trimmed}") };
    Url::parse(&candidate).map_err(|e| format!("無效的網址：{e}"))
}

/// Execute one browser tool. Value-shaped so the dispatch is testable;
/// the manager calls are the only side effects.
pub fn execute_tool(app: &AppHandle, name: &str, args: &Value) -> Result<Value, String> {
    match name {
        "activate_tab" => {
            let tab_id = args
                .get("tabId")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| "activate_tab 需要 tabId".to_owned())?;
            #[cfg(desktop)]
            crate::manager::activate_tab(app, TabId(tab_id)).map_err(|e| e.to_string())?;
            #[cfg(mobile)]
            crate::manager::activate_tab_mobile(app, TabId(tab_id)).map_err(|e| e.to_string())?;
            Ok(json!({ "ok": true, "tabId": tab_id, "tabs": tabs_value(app) }))
        }
        "open_url" => {
            let raw = args
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "open_url 需要 url 參數".to_owned())?;
            let parsed = normalize_url(raw)?;
            let new_tab = args.get("newTab").and_then(|v| v.as_bool()).unwrap_or(true);
            #[cfg(desktop)]
            {
                let window = app
                    .get_window(state::MAIN_LABEL)
                    .ok_or_else(|| "主視窗不存在".to_owned())?;
                if new_tab {
                    let id = crate::manager::open_tab(app, &window, Some(parsed.clone()));
                    Ok(json!({ "ok": true, "tabId": id.0, "opened": parsed.as_str() }))
                } else {
                    let id = {
                        let state = app.state::<BrowserState>();
                        let active = state::lock(&state.model).active_id();
                        active.ok_or_else(|| "沒有作用中的分頁".to_owned())?
                    };
                    crate::manager::navigate(app, id, parsed.as_str()).map_err(|e| e.to_string())?;
                    Ok(json!({ "ok": true, "tabId": id.0, "opened": parsed.as_str() }))
                }
            }
            #[cfg(mobile)]
            {
                if new_tab {
                    let id = crate::manager::open_tab_mobile(app, Some(parsed.clone()));
                    Ok(json!({ "ok": true, "tabId": id.0, "opened": parsed.as_str() }))
                } else {
                    let id = {
                        let state = app.state::<BrowserState>();
                        let active = state::lock(&state.model).active_id();
                        active.ok_or_else(|| "沒有作用中的分頁".to_owned())?
                    };
                    crate::manager::navigate_mobile(app, id, parsed.as_str()).map_err(|e| e.to_string())?;
                    Ok(json!({ "ok": true, "tabId": id.0, "opened": parsed.as_str() }))
                }
            }
        }
        "get_tabs" => Ok(json!({ "ok": true, "tabs": tabs_value(app) })),
        other => Err(format!("未知的瀏覽器工具：{other}")),
    }
}

// ---------------------------------------------------------------------------
// Turn loop (AG-UI endpoint)

/// What the turn loop does after one server run.
#[derive(Debug, PartialEq, Eq)]
pub enum RoundDecision {
    /// Tool calls completed and the cap allows another run.
    FollowUp,
    /// No tool calls (or none needing results) — the turn is done.
    Stop,
    /// Tool calls exist but MAX_FOLLOW_UPS is exhausted.
    CapReached,
}

pub fn round_decision(completed_tool_calls: usize, follow_ups_done: usize) -> RoundDecision {
    if completed_tool_calls == 0 {
        RoundDecision::Stop
    } else if follow_ups_done < MAX_FOLLOW_UPS {
        RoundDecision::FollowUp
    } else {
        RoundDecision::CapReached
    }
}

struct RunOutcome {
    /// Failure message, if the run did not finish cleanly.
    error: Option<String>,
    /// True when the failure was a server RUN_ERROR that run_once
    /// already forwarded verbatim; transport failures emit nothing
    /// themselves and the turn loop must raise them.
    error_emitted: bool,
    tool_calls: usize,
}

/// One POST → SSE run with injectable effects so the whole client
/// loop (HTTP, SSE parse, tool loop, thread bookkeeping) is drivable
/// against a real socket in tests. Every event forwards verbatim;
/// assistant text and tool calls land in the thread as they close;
/// browser tools execute on TOOL_CALL_END and their results (as a
/// client-produced TOOL_CALL_RESULT event plus thread messages) ride
/// the next run.
fn run_once_raw<F, X, P, A>(
    endpoint: &str,
    api_key: Option<&str>,
    input: &Value,
    mut emit: F,
    mut exec_tool: X,
    mut push_thread: P,
    alive: A,
) -> RunOutcome
where
    F: FnMut(&Value),
    X: FnMut(&ToolCall, &Value) -> (Value, bool),
    P: FnMut(Value),
    A: Fn() -> bool,
{
    let mut outcome = RunOutcome { error: None, error_emitted: false, tool_calls: 0 };
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(8))
        .timeout_read(READ_TIMEOUT)
        .timeout(TURN_TIMEOUT)
        .build();
    let mut request = agent
        .post(endpoint)
        .set("Content-Type", "application/json")
        .set("Accept", "text/event-stream");
    if let Some(key) = api_key {
        request = request.set("Authorization", &format!("Bearer {key}"));
    }
    let body = match serde_json::to_vec(input) {
        Ok(b) => b,
        Err(e) => {
            outcome.error = Some(e.to_string());
            return outcome;
        }
    };
    let resp = match request.send_bytes(&body) {
        Ok(r) => r,
        Err(e) => {
            outcome.error = Some(format!("AG-UI 連線失敗：{e}"));
            return outcome;
        }
    };
    let status = resp.status();
    if status != 200 {
        let mut text = String::new();
        use std::io::Read;
        let _ = resp.into_reader().take(4096).read_to_string(&mut text);
        outcome.error = Some(format!("AG-UI endpoint 回應 {status}：{}", text.chars().take(200).collect::<String>()));
        return outcome;
    }
    let mut reader = BufReader::new(resp.into_reader());
    let mut parser = SseParser::default();
    let mut collector = TurnCollector::default();
    let mut line = String::new();
    let mut stalls: u32 = 0;
    loop {
        if !alive() {
            return outcome; // Panel closed: stop without an error card.
        }
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break, // Stream ended (with or without RUN_FINISHED).
            Ok(_) => stalls = 0,
            Err(e) => {
                // ureq surfaces a read timeout as an io error; only
                // repeated stalls give up.
                let timed_out = matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut);
                if !timed_out {
                    outcome.error = Some(format!("SSE 讀取失敗：{e}"));
                    return outcome;
                }
                stalls += 1;
                if stalls >= MAX_READ_STALLS {
                    outcome.error = Some("AG-UI 串流逾時（伺服器停止回應）".into());
                    return outcome;
                }
                continue;
            }
        }
        let Some(payload) = parser.feed_line(&line) else { continue };
        let Ok(raw) = serde_json::from_str::<Value>(&payload) else {
            eprintln!("[agui] non-JSON SSE frame: {}", payload.chars().take(120).collect::<String>());
            continue;
        };
        // Forward verbatim first so the panel streams in real time,
        // then act on it.
        emit(&raw);
        let kind = event_kind(&raw);
        match collector.feed(kind, &raw) {
            Some(Completed::AssistantText { id, content }) => {
                push_thread(json!({ "id": id, "role": "assistant", "content": content }));
            }
            Some(Completed::ToolCall(call)) => {
                outcome.tool_calls += 1;
                let args = parse_args(&call.args_raw);
                let (result, success) = exec_tool(&call, &args);
                // Protocol shape for the next run's input: the
                // assistant proposes the call, a tool message answers.
                push_thread(json!({
                    "id": next_id("a"),
                    "role": "assistant",
                    "content": "",
                    "toolCalls": [{ "id": call.id, "name": call.name, "args": args }],
                }));
                let result_id = next_id("tr");
                push_thread(json!({
                    "id": result_id,
                    "role": "tool",
                    "toolCallId": call.id,
                    "content": result.to_string(),
                }));
                emit(&json!({
                    "type": "TOOL_CALL_RESULT",
                    "messageId": result_id,
                    "toolCallId": call.id,
                    "content": result.to_string(),
                    "role": "tool",
                    // Client-side extension so the chip can show
                    // 完成/錯誤 without parsing content.
                    "success": success,
                }));
            }
            None => {}
        }
        match kind {
            EventKind::RunFinished => break,
            EventKind::RunError => {
                outcome.error = Some(
                    raw.get("message")
                        .and_then(|m| m.as_str())
                        .map(str::to_owned)
                        .unwrap_or_else(|| "AG-UI 執行錯誤".into()),
                );
                // The raw event was already forwarded verbatim above.
                outcome.error_emitted = true;
                break;
            }
            _ => {}
        }
    }
    outcome
}

/// The production binding of `run_once_raw`: chrome events, real
/// browser tools, the global thread, the panel-open abort flag.
fn run_once(handle: &AppHandle, endpoint: &str, api_key: Option<&str>, input: &Value) -> RunOutcome {
    run_once_raw(
        endpoint,
        api_key,
        input,
        |ev| emit(handle, ev),
        |call, args| match execute_tool(handle, &call.name, args) {
            Ok(v) => (v, true),
            Err(msg) => (json!({ "error": msg }), false),
        },
        |m| thread().push(m),
        || RUN_ACTIVE.load(Ordering::SeqCst),
    )
}

fn emit_run_error(handle: &AppHandle, message: &str) {
    emit(handle, &json!({ "type": "RUN_ERROR", "message": message }));
}

/// The full user turn against an AG-UI endpoint: runs, executes tools,
/// follows up with their results until done or capped.
fn agui_turn(handle: &AppHandle, endpoint: &str, api_key: Option<String>) {
    let started = Instant::now();
    let thread_id = format!("surf-{}", std::process::id());
    let tools = browser_tools();
    let mut follow_ups = 0;
    loop {
        if !RUN_ACTIVE.load(Ordering::SeqCst) {
            return;
        }
        if started.elapsed() > TURN_TIMEOUT {
            emit_run_error(handle, "本回合超過時間上限（120 秒），已停止。");
            return;
        }
        let input = {
            let snapshot = thread().clone();
            run_input(&thread_id, &snapshot, &tools, tabs_value(handle))
        };
        let outcome = run_once(handle, endpoint, api_key.as_deref(), &input);
        if let Some(msg) = outcome.error {
            // Server RUN_ERRORs were already forwarded verbatim; only
            // transport failures need an explicit card.
            if !outcome.error_emitted && RUN_ACTIVE.load(Ordering::SeqCst) {
                emit_run_error(handle, &msg);
            }
            return;
        }
        match round_decision(outcome.tool_calls, follow_ups) {
            RoundDecision::FollowUp => follow_ups += 1,
            RoundDecision::Stop => return,
            RoundDecision::CapReached => {
                emit_run_error(handle, "已達工具回合上限（5），停止以避免無限迴圈。");
                return;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Config

/// The configured AG-UI endpoint and its optional API key. None until
/// the endpoint exists; a missing key is fine (many endpoints are
/// local and unauthenticated).
pub fn agui_config(app: &AppHandle) -> Option<(String, Option<String>)> {
    let endpoint = {
        let state = app.state::<BrowserState>();
        let db = state::lock(&state.db);
        db.ext_kv_get("sys", "setting:agent_agui_endpoint").ok().flatten()
    }?;
    if endpoint.trim().is_empty() {
        return None;
    }
    let key = {
        let state = app.state::<BrowserState>();
        state.secrets.get("agent_agui_api_key").ok().filter(|k| !k.trim().is_empty())
    };
    Some((endpoint.trim().to_owned(), key))
}

// ---------------------------------------------------------------------------
// Commands

/// Open does nothing (no auto-run; the panel greets locally). Close
/// aborts any in-flight run and SSE read.
#[tauri::command]
pub fn agent_chat_set_open(open: bool) {
    if !open {
        RUN_ACTIVE.store(false, Ordering::SeqCst);
    }
}

/// Send one user message and stream the answer. The source is AG-UI
/// endpoint > LLM > demo (the same precedence the panel status shows).
#[tauri::command]
pub fn agent_chat_send(app: AppHandle, text: String) -> Result<(), String> {
    let text = text.trim().to_owned();
    if text.is_empty() {
        return Ok(());
    }
    if RUN_ACTIVE.swap(true, Ordering::SeqCst) {
        return Err("上一則回應還在進行中，請等它結束。".into());
    }
    thread().push(json!({ "id": next_id("u"), "role": "user", "content": text }));
    let handle = app.clone();
    std::thread::spawn(move || {
        match agui_config(&handle) {
            Some((endpoint, key)) => {
                eprintln!("[agui] turn via endpoint: {endpoint}");
                agui_turn(&handle, &endpoint, key);
            }
            None => {
                eprintln!("[agui] turn via local provider (no endpoint configured)");
                // Local sources speak the same event shape.
                let provider = crate::agent::provider_for(&handle);
                provider.run_turn(&handle, &mut |ev| emit(&handle, ev));
            }
        }
        RUN_ACTIVE.store(false, Ordering::SeqCst);
    });
    Ok(())
}

/// The thread for panel-open replay (user/assistant/tool messages).
#[tauri::command]
pub fn agent_chat_history() -> Vec<Value> {
    thread().clone()
}

/// Clear the conversation and abort any in-flight run.
#[tauri::command]
pub fn agent_chat_reset() {
    RUN_ACTIVE.store(false, Ordering::SeqCst);
    thread().clear();
}

/// Read the AG-UI endpoint config. The key is never returned; `hasKey`
/// says whether one is stored.
#[tauri::command]
pub fn agent_agui_config_get(app: AppHandle) -> Value {
    let state = app.state::<BrowserState>();
    let endpoint = {
        let db = state::lock(&state.db);
        db.ext_kv_get("sys", "setting:agent_agui_endpoint").ok().flatten().unwrap_or_default()
    };
    let has_key = state.secrets.get("agent_agui_api_key").map(|k| !k.trim().is_empty()).unwrap_or(false);
    json!({ "endpoint": endpoint, "hasKey": has_key })
}

/// Save the AG-UI endpoint config. An empty key keeps the stored one;
/// `clear_key` removes it. An empty endpoint clears the setting (the
/// panel falls back to LLM/demo).
#[tauri::command]
pub fn agent_agui_config_set(
    app: AppHandle,
    endpoint: String,
    api_key: Option<String>,
    clear_key: bool,
) -> Result<(), String> {
    {
        let state = app.state::<BrowserState>();
        let db = state::lock(&state.db);
        let _ = db.ext_kv_set("sys", "setting:agent_agui_endpoint", endpoint.trim());
    }
    if clear_key {
        let state = app.state::<BrowserState>();
        let _ = state.secrets.delete("agent_agui_api_key");
    } else if let Some(key) = api_key.map(|k| k.trim().to_owned()).filter(|k| !k.is_empty()) {
        let state = app.state::<BrowserState>();
        state.secrets.put("agent_agui_api_key", &key).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_frames_assemble_multi_data_lines_and_strip_crlf() {
        // Lines arrive exactly as read_line delivers them: newline
        // included. A parser that forgets the strip never sees a blank
        // line and never dispatches a frame — the live round's bug.
        let mut p = SseParser::default();
        assert_eq!(p.feed_line("event: message\n"), None);
        assert_eq!(p.feed_line("id: 7\n"), None);
        assert_eq!(p.feed_line(": keepalive\n"), None);
        // Per SSE spec consecutive data lines join with \n; AG-UI
        // servers send one JSON frame per data line, so this documents
        // the join without endorsing split JSON.
        assert_eq!(p.feed_line("data: first\r\n"), None);
        assert_eq!(p.feed_line("data: second\n"), None);
        // The blank line dispatches the accumulated frame.
        assert_eq!(p.feed_line("\n"), Some("first\nsecond".to_owned()));
    }

    #[test]
    fn sse_blank_line_without_data_is_keepalive() {
        let mut p = SseParser::default();
        assert_eq!(p.feed_line("\n"), None);
        assert_eq!(p.feed_line("data: {}\n"), None);
        assert_eq!(p.feed_line("\n"), Some("{}".to_owned()));
    }

    #[test]
    fn sse_dispatches_one_json_frame_per_data_line() {
        let mut p = SseParser::default();
        assert_eq!(p.feed_line("data: {\"type\":\"RUN_STARTED\"}\n"), None);
        assert_eq!(p.feed_line("\n"), Some("{\"type\":\"RUN_STARTED\"}".to_owned()));
    }

    #[test]
    fn event_kind_classifies_the_consumed_set_and_tolerates_unknown() {
        for (ty, kind) in [
            ("RUN_STARTED", EventKind::RunStarted),
            ("RUN_FINISHED", EventKind::RunFinished),
            ("RUN_ERROR", EventKind::RunError),
            ("TEXT_MESSAGE_START", EventKind::TextStart),
            ("TEXT_MESSAGE_CONTENT", EventKind::TextContent),
            ("TEXT_MESSAGE_END", EventKind::TextEnd),
            ("TOOL_CALL_START", EventKind::ToolStart),
            ("TOOL_CALL_ARGS", EventKind::ToolArgs),
            ("TOOL_CALL_END", EventKind::ToolEnd),
        ] {
            assert_eq!(event_kind(&json!({ "type": ty })), kind);
        }
        assert_eq!(event_kind(&json!({ "type": "STATE_SNAPSHOT" })), EventKind::Other);
        assert_eq!(event_kind(&json!({})), EventKind::Other);
    }

    #[test]
    fn collector_accumulates_text_and_completes_on_end() {
        let mut c = TurnCollector::default();
        let m = "m1";
        assert_eq!(c.feed(EventKind::TextStart, &json!({ "messageId": m, "role": "assistant" })), None);
        assert_eq!(c.feed(EventKind::TextContent, &json!({ "messageId": m, "delta": "你" })), None);
        assert_eq!(c.feed(EventKind::TextContent, &json!({ "messageId": m, "delta": "好" })), None);
        match c.feed(EventKind::TextEnd, &json!({ "messageId": m })) {
            Some(Completed::AssistantText { id, content }) => {
                assert_eq!(id, m);
                assert_eq!(content, "你好");
            }
            other => panic!("expected text completion, got {other:?}"),
        }
        // A second END without content completes nothing.
        assert!(c.feed(EventKind::TextEnd, &json!({ "messageId": m })).is_none());
    }

    #[test]
    fn collector_assembles_streamed_tool_args() {
        let mut c = TurnCollector::default();
        c.feed(EventKind::ToolStart, &json!({ "toolCallId": "t1", "toolCallName": "open_url" }));
        c.feed(EventKind::ToolArgs, &json!({ "toolCallId": "t1", "delta": "{\"url\": " }));
        c.feed(EventKind::ToolArgs, &json!({ "toolCallId": "t1", "delta": "\"example.com\"}" }));
        match c.feed(EventKind::ToolEnd, &json!({ "toolCallId": "t1" })) {
            Some(Completed::ToolCall(call)) => {
                assert_eq!(call.id, "t1");
                assert_eq!(call.name, "open_url");
                assert_eq!(parse_args(&call.args_raw), json!({ "url": "example.com" }));
            }
            other => panic!("expected tool completion, got {other:?}"),
        }
    }

    #[test]
    fn parse_args_handles_empty_and_garbage() {
        assert_eq!(parse_args(""), json!({}));
        assert_eq!(parse_args("   "), json!({}));
        assert_eq!(parse_args("not json"), json!({ "unparsed": "not json" }));
    }

    #[test]
    fn run_input_prepends_system_and_carries_tools_state_thread() {
        let thread = vec![json!({ "id": "u-1", "role": "user", "content": "hi" })];
        let input = run_input("surf-1", &thread, &browser_tools(), json!({ "tabs": [] }));
        assert_eq!(input["threadId"], "surf-1");
        assert_eq!(input["messages"][0]["role"], "system");
        assert_eq!(input["messages"][1]["content"], "hi");
        let names: Vec<&str> = input["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["activate_tab", "open_url", "get_tabs"]);
        assert_eq!(input["state"], json!({ "tabs": [] }));
        assert!(input["context"].as_object().unwrap().is_empty());
    }

    #[test]
    fn round_decision_caps_follow_ups() {
        assert_eq!(round_decision(0, 0), RoundDecision::Stop);
        assert_eq!(round_decision(2, 0), RoundDecision::FollowUp);
        assert_eq!(round_decision(1, MAX_FOLLOW_UPS - 1), RoundDecision::FollowUp);
        assert_eq!(round_decision(1, MAX_FOLLOW_UPS), RoundDecision::CapReached);
    }

    #[test]
    fn normalize_url_accepts_bare_hosts_and_refuses_garbage() {
        assert_eq!(normalize_url("example.com/x").unwrap().as_str(), "https://example.com/x");
        assert_eq!(normalize_url("http://a.dev").unwrap().as_str(), "http://a.dev/");
        assert!(normalize_url("not a url %%%").is_err());
    }

    /// A minimal AG-UI endpoint over a real TCP socket: reads one
    /// POST, answers with a scripted SSE stream (text + one tool call),
    /// and closes. One connection per serve() call.
    fn serve_once(listener: std::net::TcpListener) -> String {
        use std::io::{Read, Write};
        let (mut sock, _) = listener.accept().unwrap();
        let mut req: Vec<u8> = Vec::new();
        let mut buf = [0u8; 8192];
        loop {
            let n = sock.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            req.extend_from_slice(&buf[..n]);
            if req.windows(4).any(|w| w == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&req).into_owned();
                let cl: usize = head
                    .lines()
                    .find(|l| l.to_ascii_lowercase().starts_with("content-length"))
                    .and_then(|l| l.split(':').nth(1))
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(0);
                let body_start = req.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
                if req.len() >= body_start + cl {
                    break;
                }
            }
        }
        let head = String::from_utf8_lossy(&req).into_owned();
        let cl: usize = head
            .lines()
            .find(|l| l.to_ascii_lowercase().starts_with("content-length"))
            .and_then(|l| l.split(':').nth(1))
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0);
        let body_start = req.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
        let body = String::from_utf8_lossy(&req[body_start..body_start + cl]).into_owned();
        // Chunked framing, the encoding real SSE servers (Node's http)
        // use for streamed responses without a known length.
        let frames = [
            "data: {\"type\":\"RUN_STARTED\"}\n\n",
            "data: {\"type\":\"TEXT_MESSAGE_START\",\"messageId\":\"m1\",\"role\":\"assistant\"}\n\n",
            "data: {\"type\":\"TEXT_MESSAGE_CONTENT\",\"messageId\":\"m1\",\"delta\":\"你好\"}\n\n",
            "data: {\"type\":\"TEXT_MESSAGE_END\",\"messageId\":\"m1\"}\n\n",
            "data: {\"type\":\"TOOL_CALL_START\",\"toolCallId\":\"t1\",\"toolCallName\":\"activate_tab\"}\n\n",
            "data: {\"type\":\"TOOL_CALL_ARGS\",\"toolCallId\":\"t1\",\"delta\":\"{\\\"tabId\\\":7}\"}\n\n",
            "data: {\"type\":\"TOOL_CALL_END\",\"toolCallId\":\"t1\"}\n\n",
            "data: {\"type\":\"RUN_FINISHED\"}\n\n",
        ];
        let mut stream =
            String::from("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n");
        for frame in frames {
            stream.push_str(&format!("{:x}\r\n{}\r\n", frame.len(), frame));
        }
        stream.push_str("0\r\n\r\n");
        sock.write_all(stream.as_bytes()).unwrap();
        let _ = sock.shutdown(std::net::Shutdown::Both);
        body
    }

    /// The full client loop against a real socket: HTTP POST carries
    /// the RunAgentInput (thread + declared tools), the SSE stream is
    /// forwarded verbatim, the tool call executes through the injected
    /// executor, and the thread gains the assistant text, the proposed
    /// tool call, and the tool result for the next run.
    #[test]
    fn sse_client_runs_full_loop_over_real_socket() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || serve_once(listener));

        let input = run_input("t-1", &[json!({ "id": "u-1", "role": "user", "content": "hi" })], &browser_tools(), json!({}));
        let mut emitted: Vec<Value> = Vec::new();
        let mut executed: Vec<(String, String, Value)> = Vec::new();
        let mut thread_log: Vec<Value> = Vec::new();
        let outcome = run_once_raw(
            &format!("http://{addr}/run"),
            None,
            &input,
            |ev| emitted.push(ev.clone()),
            |call, args| {
                executed.push((call.id.clone(), call.name.clone(), args.clone()));
                (json!({ "ok": true, "tabId": 7 }), true)
            },
            |m| thread_log.push(m),
            || true,
        );
        let body = server.join().unwrap();

        // The request carried the run input: system+user messages and
        // the declared browser tools.
        assert!(body.contains("\"threadId\":\"t-1\""), "request body: {body}");
        assert!(body.contains("activate_tab"));
        assert!(body.contains("open_url"));

        assert!(outcome.error.is_none());
        assert_eq!(outcome.tool_calls, 1);
        assert_eq!(
            executed,
            vec![("t1".to_owned(), "activate_tab".to_owned(), json!({ "tabId": 7 }))]
        );
        // Verbatim forwarding plus the client-produced TOOL_CALL_RESULT
        // before RUN_FINISHED.
        let types: Vec<&str> = emitted.iter().map(|e| e["type"].as_str().unwrap()).collect();
        assert_eq!(
            types,
            [
                "RUN_STARTED",
                "TEXT_MESSAGE_START",
                "TEXT_MESSAGE_CONTENT",
                "TEXT_MESSAGE_END",
                "TOOL_CALL_START",
                "TOOL_CALL_ARGS",
                "TOOL_CALL_END",
                "TOOL_CALL_RESULT",
                "RUN_FINISHED",
            ]
        );
        assert_eq!(emitted[7]["toolCallId"], "t1");
        assert_eq!(emitted[7]["success"], true);
        // Thread bookkeeping: text, proposed call, tool result.
        assert_eq!(thread_log.len(), 3);
        assert_eq!(thread_log[0]["role"], "assistant");
        assert_eq!(thread_log[0]["content"], "你好");
        assert_eq!(thread_log[1]["toolCalls"][0]["name"], "activate_tab");
        assert_eq!(thread_log[1]["toolCalls"][0]["args"]["tabId"], 7);
        assert_eq!(thread_log[2]["role"], "tool");
        assert_eq!(thread_log[2]["toolCallId"], "t1");
        assert!(thread_log[2]["content"].as_str().unwrap().contains("\"ok\":true"));
    }

    /// A transport failure (connection refused) surfaces as an
    /// un-emitted error the turn loop must raise.
    #[test]
    fn refused_connection_reports_transport_error_without_emitting() {
        // Port 1 on loopback: reserved, nothing listens.
        let input = run_input("t", &[], &browser_tools(), json!({}));
        let emitted: std::sync::Mutex<Vec<Value>> = std::sync::Mutex::new(Vec::new());
        let outcome = run_once_raw(
            "http://127.0.0.1:1/run",
            None,
            &input,
            |ev| emitted.lock().unwrap().push(ev.clone()),
            |_call, _args| (json!({}), true),
            |_m| {},
            || true,
        );
        assert!(outcome.error.is_some());
        assert!(!outcome.error_emitted);
        assert!(emitted.lock().unwrap().is_empty());
    }
}
