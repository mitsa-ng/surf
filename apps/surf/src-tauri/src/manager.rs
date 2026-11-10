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

/// Create a tab with a fresh id and its content webview.
pub fn open_tab(app: &AppHandle, window: &Window<Wry>, url: Option<Url>) -> TabId {
    let id = {
        let state = app.state::<BrowserState>();
        let id = state::lock(&state.model).open(url.clone());
        id
    };
    attach_webview(app, window, id, url);
    persist_session(app);
    emit_tabs(app);
    id
}

fn attach_webview(app: &AppHandle, window: &Window<Wry>, id: TabId, url: Option<Url>) {
    let state = app.state::<BrowserState>();
    let is_active = state::lock(&state.model).active_id() == Some(id);
    let label = format!("content-{}", id.0);
    let webview_url = match &url {
        Some(u) => tauri::WebviewUrl::External(u.clone()),
        None => tauri::WebviewUrl::App("start.html".into()),
    };

    let handle = app.clone();
    let tab_for_load = id;
    let title_handle = app.clone();
    let tab_for_title = id;
    let builder = tauri::webview::WebviewBuilder::new(&label, webview_url)
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
        }
        Err(e) => {
            eprintln!("failed to create content webview for {id}: {e}");
        }
    }
}

pub fn close_tab(app: &AppHandle, id: TabId) -> Result<(), crate::CommandError> {
    let state = app.state::<BrowserState>();
    let closed = state::lock(&state.model).close(id).ok_or(crate::CommandError::UnknownTab(id.0))?;
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

pub fn navigate(app: &AppHandle, id: TabId, input: &str) -> Result<(), crate::CommandError> {
    let target = match urlbox::resolve(input) {
        OmniboxResult::Navigate(url) => url,
        OmniboxResult::Search(q) => search_url(&q),
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

fn search_url(query: &str) -> Url {
    Url::parse_with_params("https://duckduckgo.com/", &[("q", query)]).unwrap_or_else(|_| state::home_url())
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
