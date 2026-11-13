use surf_core::tabs::TabId;
use surf_core::urlbox::{self, OmniboxResult};
use tauri::{AppHandle, Emitter, Manager, Window, Wry};
use url::Url;

use crate::state::{self, BrowserState};
use crate::tabview::TabView;

const TABS_EVENT: &str = "tabs://changed";

/// Label scheme for content webviews, shared by creation, lookup, the
/// token binding in commands, and subresource-filter installs.
pub fn content_label(id: TabId) -> String {
    format!("content-{}", id.0)
}

pub fn emit_tabs(app: &AppHandle) {
    let state = app.state::<BrowserState>();
    let snapshot = {
        let model = state::lock(&state.model);
        let split = *state::lock(&state.split);
        state::TabsSnapshot { tabs: model.list().to_vec(), active: model.active_id(), split }
    };
    let _ = app.emit_to("chrome", TABS_EVENT, &snapshot);
}

/// Broadcast a tabs.* event to every running extension background thread
/// that holds the "tabs" API grant (Chrome gates url/title data the same
/// way). Names map onto surf.tabs.onXxx in the script world.
pub fn notify_tabs_event(app: &AppHandle, name: &str, payload: serde_json::Value) {
    let state = app.state::<BrowserState>();
    let ids = state::lock(&state.bg_threads).ids();
    if ids.is_empty() {
        return;
    }
    let mut allowed = Vec::new();
    {
        let mut pm = state::lock(&state.permission);
        for id in &ids {
            if pm.ensure(id, &surf_ext_host::permission::PermissionRequest::Api("tabs".into())) {
                allowed.push(id.clone());
            }
        }
    }
    for id in allowed {
        state::notify_bg(
            app,
            &id,
            crate::bg::BgMsg::Envelope(surf_ext_host::router::Envelope {
                from: surf_ext_host::router::Endpoint::Host,
                to: surf_ext_host::router::Endpoint::Extension(id.clone()),
                body: surf_ext_host::router::MessageBody::Event {
                    name: name.to_owned(),
                    payload: payload.clone(),
                },
            }),
        );
    }
}

/// Journal a committed navigation into the model and refresh the
/// mirrored back/forward flags; only a flag change re-broadcasts.
#[cfg(desktop)]
fn record_navigation(app: &AppHandle, id: TabId, url: &Url) {
    let changed = {
        let state = app.state::<BrowserState>();
        // The model guard must drop before emit_tabs re-locks the model.
        let changed = state::lock(&state.model).record_navigation(id, url.clone());
        changed
    };
    if changed {
        emit_tabs(app);
    }
    notify_tabs_event(app, "tabs.onUpdated", serde_json::json!({ "id": id.0, "changeInfo": { "url": url.as_str() } }));
}

/// A load that never reaches Finished would spin the tab dot forever
/// (wry/tauri expose no failure signal, so DNS deaths and blocked frames
/// leave `loading` stuck); clear the flag after 15 seconds as the honest
/// approximation of a failed load.
fn arm_loading_watchdog(app: &AppHandle, id: TabId) {
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(15));
        let state = handle.state::<BrowserState>();
        let still = state::lock(&state.model).get(id).map(|t| t.loading).unwrap_or(false);
        if still {
            state::lock(&state.model).set_loading(id, false);
            emit_tabs(&handle);
        }
    });
}

/// Create a tab. Desktop attaches a content webview; mobile records the
/// model entry only (webview-per-tab lands in M2).
pub fn open_tab(app: &AppHandle, window: &Window<Wry>, url: Option<Url>) -> TabId {
    let id = {
        let state = app.state::<BrowserState>();
        let id = state::lock(&state.model).open(url.clone());
        id
    };
    #[cfg(desktop)]
    attach_webview(app, window, id, url);
    #[cfg(not(desktop))]
    let _ = window;
    persist_session(app);
    emit_tabs(app);
    notify_tabs_event(
        app,
        "tabs.onCreated",
        serde_json::to_value(state::lock(&app.state::<BrowserState>().model).get(id)).unwrap_or(serde_json::Value::Null),
    );
    id
}

/// Open a private (incognito) tab: no history recording, never written
/// to the persisted session.
#[cfg(desktop)]
pub fn open_private_tab(app: &AppHandle, window: &Window<Wry>, url: Option<Url>) -> TabId {
    let id = {
        let state = app.state::<BrowserState>();
        let mut model = state::lock(&state.model);
        model.open_private(url.clone())
    };
    attach_webview(app, window, id, url);
    // persist_session skips private tabs, but the previously active tab
    // may reorder positions; keep the stored session consistent.
    persist_session(app);
    emit_tabs(app);
    notify_tabs_event(
        app,
        "tabs.onCreated",
        serde_json::to_value(state::lock(&app.state::<BrowserState>().model).get(id)).unwrap_or(serde_json::Value::Null),
    );
    id
}

#[cfg(all(debug_assertions, desktop))]
fn diagnose_bridge(app: &AppHandle, id: TabId) {
    let handle = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(4));
        if let Some(view) = crate::state::content_webview(&handle, id) {
            let _ = view.eval(
                r#"document.title = "bridge:" + (document.documentElement.dataset.surfBridge || "none")"#,
            );
        } else {
            return;
        }
    });
}

/// Navigation verdict shared by desktop child webviews and mobile tab
/// windows. A blocked navigation never reaches on_page_load, so the
/// loading flag the omnibox set must be cleared here.
fn nav_verdict(app: &AppHandle, id: TabId, url: &Url) -> bool {
    let state = app.state::<BrowserState>();
    let allowed = state.nav_filter.allows(url);
    if !allowed {
        eprintln!("[nav-filter] blocked {url}");
        state::lock(&state.model).set_loading(id, false);
        emit_tabs(app);
    }
    allowed
}

/// Model bookkeeping shared by desktop child webviews and mobile tab
/// windows: loading state, committed URL, history, and the interim host
/// title until the document-title event fires.
/// Tab audio mute at the JS level: WKWebView exposes no native mute API
/// (probed: no KVC key, no private selectors on macOS 27.2), so the page's
/// media elements are muted directly and an observer keeps later ones
/// muted. Unmuting restores the elements and drops the observer.
pub(crate) const MUTE_SCRIPT: &str = r#"(function (muted) {
  if (!window.__surfMute) window.__surfMute = { on: false, obs: null };
  var s = window.__surfMute;
  s.on = muted;
  function apply(el) { try { el.muted = muted; } catch (e) {} }
  var all = document.querySelectorAll("video, audio");
  for (var i = 0; i < all.length; i++) apply(all[i]);
  if (muted && !s.obs) {
    s.obs = new MutationObserver(function (muts) {
      for (var j = 0; j < muts.length; j++) {
        var add = muts[j].addedNodes;
        for (var k = 0; k < add.length; k++) {
          var n = add[k];
          if (!n || n.nodeType !== 1) continue;
          if (n.tagName === "VIDEO" || n.tagName === "AUDIO") apply(n);
          var inner = n.querySelectorAll ? n.querySelectorAll("video, audio") : [];
          for (var q = 0; q < inner.length; q++) apply(inner[q]);
        }
      }
    });
    s.obs.observe(document.documentElement, { childList: true, subtree: true });
  }
  if (!muted && s.obs) { try { s.obs.disconnect(); } catch (e) {} s.obs = null; }
})"#;

/// Apply the tab's recorded zoom and mute state to its webview, and fill
/// a saved login for the page's host when one exists. Called on
/// load-finish because a fresh document starts at 100% and unmuted.
fn reapply_tab_media(app: &AppHandle, id: TabId) {
    let state = app.state::<BrowserState>();
    let (zoom, muted, host) = {
        let model = state::lock(&state.model);
        model
            .get(id)
            .map(|t| (t.zoom, t.muted, t.url.as_ref().and_then(|u| u.host_str().map(str::to_owned))))
            .unwrap_or((1.0, false, None))
    };
    if let Some(view) = state::content_webview(app, id) {
        if (zoom - 1.0).abs() > f64::EPSILON {
            let _ = view.raw().set_zoom(zoom);
        }
        if muted {
            let _ = view.eval(&format!("{MUTE_SCRIPT}(true);"));
        }
    }
    // Auto-fill happens only for hosts with a saved login (keychain
    // lookup); values land in the form, the submit stays with the user.
    if let Some(host) = host {
        let Ok(entry) = state.secrets.get(&format!("pw:{host}")) else {
            return;
        };
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&entry) else {
            return;
        };
        let (Some(user), Some(pass)) = (
            parsed.get("user").and_then(|v| v.as_str()),
            parsed.get("pass").and_then(|v| v.as_str()),
        ) else {
            return;
        };
        if let Some(view) = state::content_webview(app, id) {
            let _ = view.eval(&format!(
                "{}({}, {});",
                crate::commands::FILL_PW_SCRIPT,
                serde_json::to_string(user).unwrap_or_default(),
                serde_json::to_string(pass).unwrap_or_default(),
            ));
        }
    }
}

fn page_load_update(app: &AppHandle, id: TabId, url: Option<Url>, started: bool) {
    let state = app.state::<BrowserState>();
    {
        let mut model = state::lock(&state.model);
        if started {
            model.set_loading(id, true);
            if let Some(u) = &url {
                model.set_url(id, u.clone());
            }
            drop(model);
            arm_loading_watchdog(app, id);
            emit_tabs(app);
            notify_tabs_event(app, "tabs.onUpdated", serde_json::json!({ "id": id.0, "changeInfo": { "loading": true } }));
            return;
        }
        model.set_loading(id, false);
        if let Some(u) = &url {
            model.set_url(id, u.clone());
            if u.scheme() == "http" || u.scheme() == "https" {
                if let Some(host) = u.host_str() {
                    model.set_title(id, host.to_owned());
                }
                // Private tabs leave no history.
                let private = model.get(id).map(|t| t.incognito).unwrap_or(false);
                if !private {
                    let _ = state::lock(&state.db).record_visit(u, None);
                }
            }
        }
    }
    // A fresh document resets page zoom and element mute state; restore
    // the tab's recorded settings before the user sees the page.
    reapply_tab_media(app, id);
    // WKContentRuleList installs must wait for the first finished load
    // (the controller's proxy is null before the page initializes).
    crate::filters::tab_loaded(app, id);
    emit_tabs(app);
    notify_tabs_event(app, "tabs.onUpdated", serde_json::json!({ "id": id.0, "changeInfo": { "loading": false, "status": "complete" } }));
}

fn title_update(app: &AppHandle, id: TabId, title: String) {
    let state = app.state::<BrowserState>();
    let tab_url = state::lock(&state.model).get(id).and_then(|t| t.url.clone());
    state::lock(&state.model).set_title(id, title.clone());
    // History rows are written at commit (before the document title
    // exists); backfill the newest row so history shows real titles.
    if let Some(u) = tab_url {
        let _ = state::lock(&state.db).history_set_title(&u, &title);
    }
    emit_tabs(app);
    notify_tabs_event(app, "tabs.onUpdated", serde_json::json!({ "id": id.0, "changeInfo": { "title": title } }));
}

#[cfg(desktop)]
fn attach_webview(app: &AppHandle, window: &Window<Wry>, id: TabId, url: Option<Url>) {
    let state = app.state::<BrowserState>();
    let is_active = state::lock(&state.model).active_id() == Some(id);
    let tab_incognito = state::lock(&state.model).get(id).map(|t| t.incognito).unwrap_or(false);
    let label = content_label(id);
    let webview_url = match &url {
        Some(u) => tauri::WebviewUrl::External(u.clone()),
        None => tauri::WebviewUrl::App("start.html".into()),
    };
    let token = state.cs_tokens.issue(id);
    crate::content_bridge::register_content_sink(app, id);

    let handle = app.clone();
    let tab_for_load = id;
    let title_handle = app.clone();
    let tab_for_title = id;
    let download_handle = app.clone();
    let app_handle_nav = app.clone();
    let nav_tab = id;
    let builder = tauri::webview::WebviewBuilder::new(&label, webview_url)
        .initialization_script(crate::content_bridge::bootstrap_script(&token))
        // Private tabs get a real ephemeral data store (nonPersistent on
        // macOS): cookies and web storage never touch the main profile.
        .incognito(tab_incognito)
        .on_download(move |_wv, event| {
            use tauri::Emitter;
            match event {
                tauri::webview::DownloadEvent::Requested { url, destination } => {
                    // Default to a filename derived from the URL path when
                    // the engine has no suggestion.
                    if destination.file_name().is_none() {
                        let name = url
                            .path()
                            .rsplit('/')
                            .find(|s| !s.is_empty())
                            .unwrap_or("download");
                        *destination = destination.join(name);
                    }
                    let _ = download_handle.emit_to("chrome", "downloads://started", url.to_string());
                    true
                }
                tauri::webview::DownloadEvent::Finished { url, path, success } => {
                    crate::commands::record_download(
                        &download_handle,
                        url.as_str(),
                        path.as_deref(),
                        success,
                    );
                    let _ = download_handle.emit_to(
                        "chrome",
                        "downloads://finished",
                        serde_json::json!({ "url": url.to_string(), "path": path.as_ref().map(|p| p.to_string_lossy()), "success": success }),
                    );
                    success
                }
                _ => true,
            }
        })
        .on_navigation(move |url| {
            let allow = nav_verdict(&app_handle_nav, nav_tab, url);
            if allow {
                record_navigation(&app_handle_nav, nav_tab, url);
            }
            allow
        })
        .on_page_load(move |_wv, payload| {
            let url = Some(payload.url().clone());
            let started = payload.event() == tauri::webview::PageLoadEvent::Started;
            page_load_update(&handle, tab_for_load, url, started);
        })
        .on_document_title_changed(move |_wv, title| title_update(&title_handle, tab_for_title, title));

    let (pos, size) = crate::layout::content_rect(window);
    match window.add_child(builder, pos, size) {
        Ok(wv) => {
            let view = crate::tabview::TauriTabView(wv);
            if !is_active {
                let _ = view.hide();
            }
            state::lock(&state.webviews).insert(id, view);
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            crate::filters::apply_filters(app, id);
            // Attach-time loads (restored tabs, deep-linked opens) skip
            // navigate(), so the loading watchdog must arm here too.
            if url.is_some() {
                arm_loading_watchdog(app, id);
            }
            let _ = crate::layout::relayout_window(window);
            // A fresh tab means the user types next: keyboard focus belongs
            // to the chrome (address bar), not the page.
            if let Some(chrome) = app.get_webview(crate::layout::CHROME_LABEL) {
                let _ = chrome.set_focus();
            }
            #[cfg(debug_assertions)]
            diagnose_bridge(&app.clone(), id);
        }
        Err(e) => {
            eprintln!("failed to create content webview for {id}: {e}");
        }
    }
}

#[cfg(desktop)]
pub fn close_tab(app: &AppHandle, id: TabId) -> Result<(), crate::CommandError> {
    let state = app.state::<BrowserState>();
    let reopen_info = snapshot_for_reopen(&state, id);
    let closed = state::lock(&state.model).close(id).ok_or(crate::CommandError::UnknownTab(id.0))?;
    remember_closed(&state, reopen_info);
    exit_split_on_close(&state, id);
    state.cs_tokens.revoke(id);
    state::lock(&state.router).unsubscribe_all(&surf_ext_host::router::Endpoint::Content(id));
    if let Some(view) = state::take_content_webview(app, id) {
        let _ = view.close();
    }
    if let Some(new_active) = closed.new_active {
        if let Some(view) = state::content_webview(app, new_active) {
            let _ = view.show();
        }
    }
    let _ = state::lock(&state.db).delete_tab(id);
    persist_session(app);
    emit_tabs(app);
    notify_tabs_event(app, "tabs.onRemoved", serde_json::json!({ "id": id.0 }));
    Ok(())
}

#[cfg(desktop)]
pub fn activate_tab(app: &AppHandle, id: TabId) -> Result<(), crate::CommandError> {
    let state = app.state::<BrowserState>();
    if !state::lock(&state.model).activate(id) {
        return Err(crate::CommandError::UnknownTab(id.0));
    }
    exit_split_on_activate(&state, id);
    let ids: Vec<TabId> = state::lock(&state.webviews).keys().copied().collect();
    for tab_id in ids {
        if let Some(view) = state::content_webview(app, tab_id) {
            let _ = if tab_id == id { view.show() } else { view.hide() };
        }
    }
    emit_tabs(app);
    notify_tabs_event(app, "tabs.onActivated", serde_json::json!({ "id": id.0 }));
    Ok(())
}

#[cfg(desktop)]
pub fn navigate(app: &AppHandle, id: TabId, input: &str) -> Result<(), crate::CommandError> {
    let target = match urlbox::resolve(input) {
        OmniboxResult::Navigate(url) => url,
        OmniboxResult::Search(q) => search_url(app, &q),
    };
    let state = app.state::<BrowserState>();
    {
        let mut model = state::lock(&state.model);
        if model.get(id).is_none() {
            return Err(crate::CommandError::UnknownTab(id.0));
        }
        model.set_url(id, target.clone());
        model.set_loading(id, true);
    }
    if let Some(view) = state::content_webview(app, id) {
        let _ = view.load(&target);
    }
    arm_loading_watchdog(app, id);
    emit_tabs(app);
    Ok(())
}

#[cfg(desktop)]
pub fn eval_history(app: &AppHandle, id: TabId, js: &str) -> Result<(), crate::CommandError> {
    let state = app.state::<BrowserState>();
    if state::lock(&state.model).get(id).is_none() {
        return Err(crate::CommandError::UnknownTab(id.0));
    }
    if let Some(view) = state::content_webview(app, id) {
        let _ = view.eval(js);
    }
    Ok(())
}

/// Capture what reopen needs before the tab leaves the model; private
/// tabs leave no trace.
fn snapshot_for_reopen(state: &BrowserState, id: TabId) -> Option<crate::state::ClosedTabInfo> {
    let model = state::lock(&state.model);
    let tab = model.get(id)?;
    if tab.incognito {
        return None;
    }
    Some(crate::state::ClosedTabInfo { url: tab.url.clone(), title: tab.title.clone() })
}

/// Record a closed tab for reopen (Cmd+Shift+T).
fn remember_closed(state: &BrowserState, info: Option<crate::state::ClosedTabInfo>) {
    let Some(info) = info else { return };
    let mut stack = state::lock(&state.recently_closed);
    stack.push(info);
    let len = stack.len();
    if len > 20 {
        stack.drain(0..len - 20);
    }
}

/// Split view survives only while both members exist.
fn exit_split_on_close(state: &BrowserState, id: TabId) {
    let mut split = state::lock(&state.split);
    if matches!(*split, Some((a, b)) if a == id || b == id) {
        *split = None;
    }
}

/// Activating a tab outside the split leaves split view: the simplest
/// predictable exit.
fn exit_split_on_activate(state: &BrowserState, id: TabId) {
    let mut split = state::lock(&state.split);
    if matches!(*split, Some((a, b)) if a != id && b != id) {
        *split = None;
    }
}

fn search_url(app: &AppHandle, query: &str) -> Url {
    let (engine, custom) = {
        let state = app.state::<BrowserState>();
        let db = state::lock(&state.db);
        let engine = db.ext_kv_get("sys", "setting:search_engine").ok().flatten();
        let custom = db.ext_kv_get("sys", "setting:search_engine_custom").ok().flatten();
        (engine.unwrap_or_else(|| "duckduckgo".into()), custom)
    };
    match urlbox::search_url(&engine, custom.as_deref(), query) {
        OmniboxResult::Navigate(u) => u,
        // A broken custom template falls back to the default engine so the
        // omnibox still searches somewhere.
        OmniboxResult::Search(q) => match urlbox::search_url("duckduckgo", None, &q) {
            OmniboxResult::Navigate(u) => u,
            OmniboxResult::Search(_) => state::home_url(),
        },
    }
}

pub fn persist_session(app: &AppHandle) {
    let state = app.state::<BrowserState>();
    let model = state::lock(&state.model);
    let split = *state::lock(&state.split);
    // Map tab id -> persisted position; split members survive the restart
    // as positions, since tab ids are reissued on restore.
    let mut positions = std::collections::HashMap::new();
    let db_result = (|| -> Result<(), surf_core::storage::StorageError> {
        let db = state::lock(&state.db);
        db.clear_tabs()?;
        let mut position = 0;
        for tab in model.list().iter() {
            // Private tabs are never written to the stored session.
            if tab.incognito {
                continue;
            }
            positions.insert(tab.id, position);
            db.save_tab(position, tab)?;
            position += 1;
        }
        if let Some((a, b)) = split {
            if let (Some(pa), Some(pb)) = (positions.get(&a), positions.get(&b)) {
                let json = serde_json::to_string(&[pa, pb]).unwrap_or_default();
                let _ = db.ext_kv_set("sys", "session:split", &json);
            } else {
                // A private (unpersisted) member cannot restore; drop it.
                let _ = db.ext_kv_set("sys", "session:split", "");
            }
        } else {
            let _ = db.ext_kv_set("sys", "session:split", "");
        }
        Ok(())
    })();
    if let Err(e) = db_result {
        eprintln!("session persist failed: {e}");
    }
}

/// Restore the persisted split (positions from `persist_session`) onto the
/// freshly reissued tab ids. Returns the split pair when both survive.
pub fn restore_split(state: &BrowserState, id_by_position: &std::collections::HashMap<usize, TabId>) -> Option<(TabId, TabId)> {
    let raw = state::lock(&state.db)
        .ext_kv_get("sys", "session:split")
        .ok()
        .flatten()
        .unwrap_or_default();
    let pair: Vec<usize> = serde_json::from_str(&raw).ok()?;
    if pair.len() != 2 {
        return None;
    }
    let (a, b) = (*id_by_position.get(&pair[0])?, *id_by_position.get(&pair[1])?);
    *state::lock(&state.split) = Some((a, b));
    Some((a, b))
}

// Mobile instance-swap: one WebviewWindow per tab. Tauri mobile supports
// multiple windows; switching shows the target so it takes the stack
// front. Windows are created lazily — restoring a session only materialises
// the active tab.

#[cfg(mobile)]
fn tab_window(app: &AppHandle, id: TabId) -> Option<tauri::WebviewWindow<Wry>> {
    app.get_webview_window(&content_label(id))
}

#[cfg(mobile)]
pub fn ensure_tab_window(app: &AppHandle, id: TabId, url: Option<Url>) -> Result<tauri::WebviewWindow<Wry>, tauri::Error> {
    if let Some(w) = tab_window(app, id) {
        return Ok(w);
    }
    let state = app.state::<BrowserState>();
    let token = state.cs_tokens.issue(id);
    drop(state);
    crate::content_bridge::register_content_sink(app, id);
    let window_url = match &url {
        Some(u) => tauri::WebviewUrl::External(u.clone()),
        None => tauri::WebviewUrl::App("start.html".into()),
    };
    let nav_handle = app.clone();
    let nav_tab = id;
    let load_handle = app.clone();
    let title_handle = app.clone();
    tauri::WebviewWindowBuilder::new(app, content_label(id), window_url)
        .title("Surf")
        .initialization_script(crate::content_bridge::bootstrap_script(&token))
        .on_navigation(move |url| nav_verdict(&nav_handle, nav_tab, url))
        .on_page_load(move |_w, payload| {
            let url = Some(payload.url().clone());
            let started = payload.event() == tauri::webview::PageLoadEvent::Started;
            page_load_update(&load_handle, id, url, started);
        })
        .on_document_title_changed(move |_w, title| title_update(&title_handle, id, title))
        .build()?;
    #[cfg(target_os = "ios")]
    crate::filters::apply_filters(app, id);
    Ok(tab_window(app, id).expect("window just built"))
}

#[cfg(mobile)]
pub fn model_open(app: &AppHandle, url: Option<Url>) -> TabId {
    let state = app.state::<BrowserState>();
    let id = state::lock(&state.model).open(url.clone());
    if let Some(u) = &url {
        state::lock(&state.model).set_url(id, u.clone());
    }
    id
}

#[cfg(mobile)]
pub fn open_tab_mobile(app: &AppHandle, url: Option<Url>) -> TabId {
    let id = model_open(app, url.clone());
    let _ = ensure_tab_window(app, id, url);
    persist_session(app);
    emit_tabs(app);
    notify_tabs_event(
        app,
        "tabs.onCreated",
        serde_json::to_value(state::lock(&app.state::<BrowserState>().model).get(id)).unwrap_or(serde_json::Value::Null),
    );
    id
}

#[cfg(mobile)]
pub fn close_tab_mobile(app: &AppHandle, id: TabId) -> Result<(), crate::CommandError> {
    let state = app.state::<BrowserState>();
    let reopen_info = snapshot_for_reopen(&state, id);
    let closed = state::lock(&state.model)
        .close(id)
        .ok_or(crate::CommandError::UnknownTab(id.0))?;
    remember_closed(&state, reopen_info);
    exit_split_on_close(&state, id);
    state.cs_tokens.revoke(id);
    state::lock(&state.router).unsubscribe_all(&surf_ext_host::router::Endpoint::Content(id));
    drop(state);
    if let Some(w) = tab_window(app, id) {
        let _ = w.close();
    }
    if let Some(next) = closed.new_active {
        if let Some(w) = tab_window(app, next) {
            let _ = w.show();
        }
    }
    persist_session(app);
    emit_tabs(app);
    notify_tabs_event(app, "tabs.onRemoved", serde_json::json!({ "id": id.0 }));
    Ok(())
}

#[cfg(mobile)]
pub fn activate_tab_mobile(app: &AppHandle, id: TabId) -> Result<(), crate::CommandError> {
    let state = app.state::<BrowserState>();
    let url = {
        if !state::lock(&state.model).activate(id) {
            return Err(crate::CommandError::UnknownTab(id.0));
        }
        state::lock(&state.model).get(id).and_then(|t| t.url.clone())
    };
    drop(state);
    match tab_window(app, id) {
        Some(w) => {
            let _ = w.show();
        }
        None => {
            if let Err(e) = ensure_tab_window(app, id, url) {
                eprintln!("mobile tab window create failed: {e}");
            }
        }
    }
    emit_tabs(app);
    notify_tabs_event(app, "tabs.onActivated", serde_json::json!({ "id": id.0 }));
    Ok(())
}

#[cfg(mobile)]
pub fn navigate_mobile(app: &AppHandle, id: TabId, input: &str) -> Result<(), crate::CommandError> {
    let target = match urlbox::resolve(input) {
        OmniboxResult::Navigate(url) => url,
        OmniboxResult::Search(q) => search_url(app, &q),
    };
    let state = app.state::<BrowserState>();
    {
        let mut model = state::lock(&state.model);
        if model.get(id).is_none() {
            return Err(crate::CommandError::UnknownTab(id.0));
        }
        model.set_url(id, target.clone());
        model.set_loading(id, true);
    }
    drop(state);
    if let Some(w) = tab_window(app, id) {
        let _ = w.eval(format!("location.replace({:?})", target.as_str()));
    }
    emit_tabs(app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::BrowserState;

    #[test]
    fn closed_tab_is_captured_before_removal_and_privates_are_skipped() {
        let db = surf_core::storage::Db::open(":memory:").unwrap();
        let dir = std::env::temp_dir().join(format!("surf-test-favicons-{}", std::process::id()));
        let state = BrowserState::new(db, dir);
        let url = Url::parse("https://closed.example/").unwrap();
        let normal = state::lock(&state.model).open(Some(url.clone()));
        let private = state::lock(&state.model).open_private(Some(url.clone()));

        let info = super::snapshot_for_reopen(&state, normal).unwrap();
        assert_eq!(info.url, Some(url.clone()));
        assert!(super::snapshot_for_reopen(&state, private).is_none());

        state::lock(&state.model).close(normal).unwrap();
        super::remember_closed(&state, Some(info));
        let stack_len = state::lock(&state.recently_closed).len();
        assert_eq!(stack_len, 1);

        // A second remember without a captured info (private close) is a no-op.
        super::remember_closed(&state, None);
        assert_eq!(state::lock(&state.recently_closed).len(), 1);
    }
}
