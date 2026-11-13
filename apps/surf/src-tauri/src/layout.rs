use surf_core::tabs::TabId;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager, Window, Wry};

use crate::state::{self, BrowserState};

/// Chrome-parity chrome stack: 40px of tabs plus a 47px toolbar. The
/// extra toolbar room is the omnibox's breathing space (6px above, 7px
/// below the 34px field), measured off Chrome on macOS.
pub const TAB_STRIP_HEIGHT_LOGICAL: f64 = 40.0;
pub const TOOLBAR_HEIGHT_LOGICAL: f64 = 47.0;
pub const CHROME_HEIGHT_LOGICAL: f64 = TAB_STRIP_HEIGHT_LOGICAL + TOOLBAR_HEIGHT_LOGICAL;
/// The optional bookmarks bar row below the toolbar.
pub const BOOKMARKS_BAR_HEIGHT_LOGICAL: f64 = 28.0;

pub const CHROME_LABEL: &str = "chrome";

/// Total chrome height: toolbar stack plus the bookmarks bar when it is
/// visible. The UI header must render exactly this height.
pub fn chrome_height(state: &BrowserState) -> f64 {
    let bar = if cfg!(desktop) && state.bookmarks_bar.load(Ordering::Relaxed) {
        BOOKMARKS_BAR_HEIGHT_LOGICAL
    } else {
        0.0
    };
    CHROME_HEIGHT_LOGICAL + bar
}

#[cfg(desktop)]
pub fn relayout(app: &AppHandle) -> Result<(), tauri::Error> {
    let Some(window) = app.get_window(state::MAIN_LABEL) else {
        return Ok(());
    };
    relayout_window(&window)
}

#[cfg(desktop)]
pub fn relayout_window(window: &Window<Wry>) -> Result<(), tauri::Error> {
    let app = window.app_handle();
    let scale = window.scale_factor().unwrap_or(1.0);
    let size = window.inner_size()?;
    let logical_size = size.to_logical::<f64>(scale);
    let state = app.state::<BrowserState>();
    let expanded = state.panel_open.load(Ordering::Relaxed)
        || state.overlay_open.load(Ordering::Relaxed);
    let chrome_h = if expanded { logical_size.height } else { chrome_height(&state) };

    let chrome = app.get_webview(CHROME_LABEL);
    if let Some(chrome) = &chrome {
        chrome.set_position(tauri::LogicalPosition::new(0.0, 0.0))?;
        chrome.set_size(tauri::LogicalSize::new(logical_size.width, chrome_h))?;
    }

    let ids: Vec<TabId> = state::lock(&state.webviews).keys().copied().collect();
    let content_top = chrome_height(&state);
    let content_height = (logical_size.height - content_top).max(0.0);
    let split = *state::lock(&state.split);
    for id in ids {
        let Some(view) = state::content_webview(app, id) else { continue };
        match split {
            // Split view: the two members share the content area; every
            // other webview hides.
            Some((left, _)) if id == left => {
                view.raw().set_position(tauri::LogicalPosition::new(0.0, content_top))?;
                view.raw().set_size(tauri::LogicalSize::new(logical_size.width / 2.0, content_height))?;
                let _ = view.show();
            }
            Some((_, right)) if id == right => {
                view.raw().set_position(tauri::LogicalPosition::new(logical_size.width / 2.0, content_top))?;
                let half = logical_size.width - (logical_size.width / 2.0);
                view.raw().set_size(tauri::LogicalSize::new(half, content_height))?;
                let _ = view.show();
            }
            Some(_) => {
                let _ = view.hide();
            }
            None => {
                view.raw().set_position(tauri::LogicalPosition::new(0.0, content_top))?;
                view.raw().set_size(tauri::LogicalSize::new(logical_size.width, content_height))?;
            }
        }
    }
    if expanded {
        if let Some(chrome) = chrome {
            // Re-adding the chrome child moves its native view above the
            // content webviews, where CSS z-index cannot reach.
            chrome.reparent(window)?;
        }
    }
    Ok(())
}

/// Chrome overlays (tab menu, tab search, address suggestions) render
/// inside the chrome webview and may extend below the chrome stack;
/// while any is open the chrome webview covers the window transparently.
#[tauri::command]
pub fn overlay_set_open(app: AppHandle, open: bool) -> Result<(), String> {
    let state = app.state::<BrowserState>();
    state.overlay_open.store(open, Ordering::Relaxed);
    #[cfg(desktop)]
    {
        relayout(&app).map_err(|error| error.to_string())
    }
    #[cfg(mobile)]
    {
        Ok(())
    }
}

#[tauri::command]
pub fn panel_set_open(app: AppHandle, open: bool) -> Result<(), String> {
    let state = app.state::<BrowserState>();
    state.panel_open.store(open, Ordering::Relaxed);
    #[cfg(desktop)]
    {
        relayout(&app).map_err(|error| error.to_string())
    }
    #[cfg(mobile)]
    {
        Ok(())
    }
}

/// Toggle the desktop bookmarks bar: persist the choice, update the
/// chrome height, and reposition the content webviews.
#[tauri::command]
pub fn bookmarks_bar_set_open(app: AppHandle, open: bool) -> Result<(), String> {
    let state = app.state::<BrowserState>();
    state.bookmarks_bar.store(open, Ordering::Relaxed);
    {
        let db = state::lock(&state.db);
        let value = if open { "1" } else { "" };
        if let Err(e) = db.ext_kv_set("sys", "setting:bookmarks_bar", value) {
            return Err(e.to_string());
        }
    }
    #[cfg(desktop)]
    {
        relayout(&app).map_err(|error| error.to_string())
    }
    #[cfg(mobile)]
    {
        Ok(())
    }
}

/// Enter split view: show two tabs side by side under the chrome.
/// Desktop only; both tabs must exist and differ.
#[tauri::command]
pub fn split_set(app: AppHandle, left: u64, right: u64) -> Result<(), String> {
    #[cfg(desktop)]
    {
        let (left, right) = (TabId(left), TabId(right));
        if left == right {
            return Err("split needs two different tabs".into());
        }
        let state = app.state::<BrowserState>();
        {
            let model = state::lock(&state.model);
            if model.get(left).is_none() || model.get(right).is_none() {
                return Err("unknown tab".into());
            }
            *state::lock(&state.split) = Some((left, right));
        }
        // Split survives restarts as part of the persisted session.
        crate::manager::persist_session(&app);
        relayout(&app).map_err(|error| error.to_string())
    }
    #[cfg(mobile)]
    {
        let _ = (app, left, right);
        Err("分割畫面目前僅支援桌面版".into())
    }
}

/// Leave split view.
#[tauri::command]
pub fn split_clear(app: AppHandle) -> Result<(), String> {
    let state = app.state::<BrowserState>();
    let had = state::lock(&state.split).take().is_some();
    #[cfg(desktop)]
    {
        if had {
            crate::manager::persist_session(&app);
            return relayout(&app).map_err(|error| error.to_string());
        }
    }
    #[cfg(mobile)]
    let _ = had;
    Ok(())
}

/// Geometry for a freshly created content webview, so it lands correctly
/// before the first resize event.
#[cfg(desktop)]
pub fn content_rect(
    window: &Window<Wry>,
) -> (tauri::LogicalPosition<f64>, tauri::LogicalSize<f64>) {
    let scale = window.scale_factor().unwrap_or(1.0);
    let size = window.inner_size().unwrap_or_default();
    let logical_size = size.to_logical::<f64>(scale);
    let state = window.app_handle().state::<BrowserState>();
    let content_top = chrome_height(&state);
    let height = (logical_size.height - content_top).max(0.0);
    (
        tauri::LogicalPosition::new(0.0, content_top),
        tauri::LogicalSize::new(logical_size.width, height),
    )
}
