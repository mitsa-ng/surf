use surf_core::tabs::TabId;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager, Window, Wry};

use crate::state::{self, BrowserState};

/// The chrome webview is 36px of tabs plus 44px of toolbar.
pub const CHROME_HEIGHT_LOGICAL: f64 = 80.0;

pub const CHROME_LABEL: &str = "chrome";

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
    let panel_open = state.panel_open.load(Ordering::Relaxed);
    let chrome_h = if panel_open { logical_size.height } else { CHROME_HEIGHT_LOGICAL };

    let chrome = app.get_webview(CHROME_LABEL);
    if let Some(chrome) = &chrome {
        chrome.set_position(tauri::LogicalPosition::new(0.0, 0.0))?;
        chrome.set_size(tauri::LogicalSize::new(logical_size.width, chrome_h))?;
    }

    let ids: Vec<TabId> = state::lock(&state.webviews).keys().copied().collect();
    let content_top = CHROME_HEIGHT_LOGICAL;
    let content_height = (logical_size.height - content_top).max(0.0);
    for id in ids {
        if let Some(view) = state::content_webview(app, id) {
            view.raw().set_position(tauri::LogicalPosition::new(0.0, content_top))?;
            view.raw().set_size(tauri::LogicalSize::new(logical_size.width, content_height))?;
        }
    }
    if panel_open {
        if let Some(chrome) = chrome {
            // Re-adding the chrome child moves its native view above the
            // content webviews, where CSS z-index cannot reach.
            chrome.reparent(window)?;
        }
    }
    Ok(())
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

/// Geometry for a freshly created content webview, so it lands correctly
/// before the first resize event.
#[cfg(desktop)]
pub fn content_rect(
    window: &Window<Wry>,
) -> (tauri::LogicalPosition<f64>, tauri::LogicalSize<f64>) {
    let scale = window.scale_factor().unwrap_or(1.0);
    let size = window.inner_size().unwrap_or_default();
    let logical_size = size.to_logical::<f64>(scale);
    let chrome_h = CHROME_HEIGHT_LOGICAL;
    let content_top = chrome_h;
    let height = (logical_size.height - content_top).max(0.0);
    (
        tauri::LogicalPosition::new(0.0, content_top),
        tauri::LogicalSize::new(logical_size.width, height),
    )
}
