use surf_core::tabs::TabId;
use surf_core::urlbox::{self, OmniboxResult};
use tauri::{AppHandle, Emitter, Manager, Window, Wry};
use url::Url;

use crate::state::{self, BrowserState};
use crate::tabview::TabView;

const TABS_EVENT: &str = "tabs://changed";

pub fn emit_tabs(app: &AppHandle) {
    let state = app.state::<BrowserState>();
    let snapshot = {
        let model = state::lock(&state.model);
        state::TabsSnapshot { tabs: model.list().to_vec(), active: model.active_id() }
    };
    let _ = app.emit_to("chrome", TABS_EVENT, &snapshot);
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

#[cfg(desktop)]
fn attach_webview(app: &AppHandle, window: &Window<Wry>, id: TabId, url: Option<Url>) {
    let state = app.state::<BrowserState>();
    let is_active = state::lock(&state.model).active_id() == Some(id);
    let label = format!("content-{}", id.0);
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
    let builder = tauri::webview::WebviewBuilder::new(&label, webview_url)
        .initialization_script(crate::content_bridge::bootstrap_script(&token))
        .on_page_load(move |_wv, payload| {
            let app = handle.clone();
            let url = Some(payload.url().clone());
            let started = payload.event() == tauri::webview::PageLoadEvent::Started;
            let state = app.state::<BrowserState>();
            {
                let mut model = state::lock(&state.model);
                if started {
                    model.set_loading(tab_for_load, true);
                    if let Some(u) = &url {
                        model.set_url(tab_for_load, u.clone());
                    }
                } else {
                    model.set_loading(tab_for_load, false);
                    if let Some(u) = &url {
                        model.set_url(tab_for_load, u.clone());
                        // Interim title until the document-title event fires.
                        if u.scheme() == "http" || u.scheme() == "https" {
                            if let Some(host) = u.host_str() {
                                model.set_title(tab_for_load, host.to_owned());
                            }
                            let _ = state::lock(&state.db).record_visit(u, None);
                        }
                    }
                }
            }
            emit_tabs(&app);
        })
        .on_document_title_changed(move |_wv, title| {
            let app = title_handle.clone();
            let state = app.state::<BrowserState>();
            state::lock(&state.model).set_title(tab_for_title, title);
            emit_tabs(&app);
        });

    let (pos, size) = crate::layout::content_rect(window);
    match window.add_child(builder, pos, size) {
        Ok(wv) => {
            let view = crate::tabview::TauriTabView(wv);
            if !is_active {
                let _ = view.hide();
            }
            state::lock(&state.webviews).insert(id, view);
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
    let closed = state::lock(&state.model).close(id).ok_or(crate::CommandError::UnknownTab(id.0))?;
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
    Ok(())
}

#[cfg(desktop)]
pub fn activate_tab(app: &AppHandle, id: TabId) -> Result<(), crate::CommandError> {
    let state = app.state::<BrowserState>();
    if !state::lock(&state.model).activate(id) {
        return Err(crate::CommandError::UnknownTab(id.0));
    }
    let ids: Vec<TabId> = state::lock(&state.webviews).keys().copied().collect();
    for tab_id in ids {
        if let Some(view) = state::content_webview(app, tab_id) {
            let _ = if tab_id == id { view.show() } else { view.hide() };
        }
    }
    emit_tabs(app);
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

fn search_url(app: &AppHandle, query: &str) -> Url {
    let engine = {
        let state = app.state::<BrowserState>();
        let db = state::lock(&state.db);
        let value = db.ext_kv_get("sys", "setting:search_engine").ok().flatten();
        value.unwrap_or_else(|| "duckduckgo".into())
    };
    let base = match engine.as_str() {
        "google" => "https://www.google.com/search",
        "bing" => "https://www.bing.com/search",
        _ => "https://duckduckgo.com/",
    };
    Url::parse_with_params(base, &[("q", query)]).unwrap_or_else(|_| state::home_url())
}

pub fn persist_session(app: &AppHandle) {
    let state = app.state::<BrowserState>();
    let model = state::lock(&state.model);
    let db_result = (|| -> Result<(), surf_core::storage::StorageError> {
        let db = state::lock(&state.db);
        db.clear_tabs()?;
        for (position, tab) in model.list().iter().enumerate() {
            db.save_tab(position, tab)?;
        }
        Ok(())
    })();
    if let Err(e) = db_result {
        eprintln!("session persist failed: {e}");
    }
}
