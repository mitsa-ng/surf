//! Agent-to-User Interface (a2ui) provider pipeline.
//!
//! The chrome renders agent UIs with the a2ui web components; this module
//! is the Rust side of the stream. An `AgentProvider` turns browser state
//! into a2ui v0.9 protocol messages and pushes them to the chrome webview
//! as `agent://message` events. This round ships the deterministic
//! `DemoProvider` (real tab data, no LLM); MCP, remote SSE, and built-in
//! LLM providers slot in behind the same trait later.

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

/// Deterministic demo agent: reads the live tab model and streams a
/// card of the real open tabs. Display components only — buttons and
/// inputs need the real agent action loop, which is a later round.
pub struct DemoProvider;

impl AgentProvider for DemoProvider {
    fn plan(&self, app: &AppHandle) -> Vec<Value> {
        let state = app.state::<BrowserState>();
        let tabs = state::lock(&state.model).list().to_vec();
        demo_messages(&tabs.iter().map(|t| (t.title.clone(), t.url.clone())).collect::<Vec<_>>())
    }
}

/// The message plan for a tab-summary turn. Pure so the shape and the
/// streaming cadence are unit-testable without a window.
fn demo_messages(tabs: &[(Option<String>, Option<Url>)]) -> Vec<Value> {
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
        let ids: Vec<String> = (0..tabs.len()).map(|i| format!("tab{i}")).collect();
        for (i, (t, u)) in tabs.iter().enumerate() {
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
            components.push(json!({ "id": format!("tab{i}"), "component": "Text", "text": text }));
        }
        components.push(json!({ "id": "list", "component": "List", "children": ids, "direction": "vertical" }));
    }
    components.push(json!({ "id": "note", "component": "Text", "text": "示範串流：規則型 agent，尚未接 LLM。", "variant": "caption" }));

    vec![
        json!({
            "version": "v0.9",
            "createSurface": {
                "surfaceId": SURFACE_ID,
                "catalogId": BASIC_CATALOG_ID,
                "theme": { "primaryColor": "#0a7fa8", "agentDisplayName": "Surf Demo Agent" },
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
    if STREAM_RUNNING.swap(true, Ordering::SeqCst) {
        return Ok(()); // A stream is already feeding the open panel.
    }
    let handle = Arc::new(app.clone());
    let provider = DemoProvider;
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn plan_shapes_valid_v0_9_messages() {
        let tabs = vec![
            (Some("Example Domain".into()), Some(url("https://example.org/"))),
            (None, Some(url("https://a.example.com/x"))),
            (None, None),
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
            .chain(root["children"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()))
            .chain(
                components.as_array().unwrap().iter().flat_map(|c| -> Vec<&str> {
                    c["children"]
                        .as_array()
                        .map(|a| a.iter().map(|v| v.as_str().unwrap()).collect())
                        .unwrap_or_default()
                }),
            )
            .map(|s| s.to_string())
            .collect();
        for r in &referenced {
            assert!(ids.contains(&r.as_str()), "unresolved child reference {r}");
        }
        // One list entry per tab, real labels only.
        let list = components.as_array().unwrap().iter().find(|c| c["id"] == "list").unwrap();
        assert_eq!(list["children"].as_array().unwrap().len(), 3);
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
    }

    #[test]
    fn ticks_space_the_stream() {
        assert!(tick_for(0) < tick_for(1));
    }
}
