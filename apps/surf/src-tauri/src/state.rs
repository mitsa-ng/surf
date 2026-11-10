use std::collections::HashMap;
use std::sync::Mutex;

use surf_core::storage::Db;
use surf_core::tabs::{TabId, TabModel, TabState};
use surf_ext_host::Registry;
use tauri::{AppHandle, Webview, WebviewWindowBuilder, Window, WindowBuilder, Wry};
use url::Url;

use crate::layout;

#[derive(serde::Serialize, Clone)]
pub struct TabsSnapshot {
    pub tabs: Vec<TabState>,
    pub active: Option<TabId>,
}

pub struct BrowserState {
    pub model: Mutex<TabModel>,
    pub db: Mutex<Db>,
    pub registry: Mutex<Registry>,
    pub webviews: Mutex<HashMap<TabId, TauriTabViewHandle>>,
}

/// Cloneable handle to a content webview. Tauri webviews are cheap
/// channel handles, safe to clone and hold.
#[derive(Clone)]
pub struct TauriTabViewHandle(pub Webview<Wry>);

impl BrowserState {
    pub fn new(db: Db) -> Self {
        Self {
            model: Mutex::new(TabModel::new()),
            db: Mutex::new(db),
            registry: Mutex::new(Registry::new()),
            webviews: Mutex::new(HashMap::new()),
        }
    }
}

pub fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Main window label; also the chrome webview label on desktop.
pub const MAIN_LABEL: &str = "main";

pub fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();

    #[cfg(desktop)]
    {
        let window = WindowBuilder::new(app, MAIN_LABEL)
            .title("Surf")
            .inner_size(1200.0, 800.0)
            .min_inner_size(480.0, 480.0)
            .build()?;

        let size = window.inner_size()?;
        let scale = window.scale_factor();
        window.add_child(
            tauri::webview::WebviewBuilder::new("chrome", tauri::WebviewUrl::App("index.html".into())),
            tauri::LogicalPosition::new(0.0, 0.0),
            tauri::LogicalSize::new(size.width as f64 / scale, layout::CHROME_HEIGHT),
        )?;

        restore_session(&handle, &window);
        watch_resize(&handle, &window);
    }

    #[cfg(mobile)]
    {
        WebviewWindowBuilder::new(app, MAIN_LABEL, tauri::WebviewUrl::App("index.html".into()))
            .title("Surf")
            .build()?;
    }

    Ok(())
}

/// Reopen the persisted session. Fresh tab ids are fine: nothing outside
/// this process references them.
fn restore_session(app: &AppHandle, window: &Window<Wry>) {
    let state = app.state::<BrowserState>();
    let saved = { lock(&state.db).load_tabs().unwrap_or_default() };
    if saved.is_empty() {
        crate::manager::open_tab(app, window, None);
        return;
    }
    for tab in saved {
        let id = crate::manager::open_tab(app, window, tab.url.clone());
        if let Some(title) = tab.title {
            lock(&state.model).set_title(id, title);
        }
    }
    // open_tab leaves the last one active, matching persisted order.
    crate::manager::emit_tabs(app);
}

fn watch_resize(app: &AppHandle, window: &Window<Wry>) {
    let handle = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Resized(_) = event {
            let _ = layout::relayout(&handle);
        }
    });
}

pub fn content_webview(app: &AppHandle, id: TabId) -> Option<Webview<Wry>> {
    let state = app.state::<BrowserState>();
    lock(&state.webviews).get(&id).map(|h| h.0.clone())
}

pub fn home_url() -> Url {
    Url::parse("https://duckduckgo.com/").unwrap()
}
