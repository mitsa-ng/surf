use surf_core::tabs::TabId;
use tauri::{AppHandle, Manager, Window, Wry};

use crate::state::{self, BrowserState};

/// Desktop tab geometry. Chrome strip on top, content fills the rest.
/// Mobile chrome lives at the bottom of the UI layout instead and has no
/// content webviews in M1.
pub const CHROME_HEIGHT: f64 = 80.0;

pub const CHROME_LABEL: &str = "chrome";

/// On macOS the webview coordinate space starts at the window's top-left,
/// behind the title bar; child webviews must be pushed below it or the
/// top of the chrome hides under the traffic lights. Measured, not
/// hardcoded, so it follows whatever title bar style the platform uses.
pub fn titlebar_offset(window: &Window<Wry>) -> f64 {
    let scale = window.scale_factor().unwrap_or(1.0);
    let outer = window.outer_size().map(|s| s.to_logical::<f64>(scale)).unwrap_or_default();
    let inner = window.inner_size().map(|s| s.to_logical::<f64>(scale)).unwrap_or_default();
    (outer.height - inner.height).max(0.0)
}

pub fn relayout(app: &AppHandle) -> Result<(), tauri::Error> {
    let Some(window) = app.get_window(state::MAIN_LABEL) else {
        return Ok(());
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    let offset = titlebar_offset(&window);
    let size = window.inner_size()?;
    let width = size.width as f64 / scale;
    let height = size.height as f64 / scale;

    if let Some(chrome) = app.get_webview(CHROME_LABEL) {
        chrome.set_position(tauri::LogicalPosition::new(0.0, offset))?;
        chrome.set_size(tauri::LogicalSize::new(width, CHROME_HEIGHT))?;
    }

    let state = app.state::<BrowserState>();
    let ids: Vec<TabId> = state::lock(&state.webviews).keys().copied().collect();
    let content_top = offset + CHROME_HEIGHT;
    let content_height = (height - content_top).max(0.0);
    for id in ids {
        if let Some(view) = state::content_webview(app, id) {
            view.raw().set_position(tauri::LogicalPosition::new(0.0, content_top))?;
            view.raw().set_size(tauri::LogicalSize::new(width, content_height))?;
        }
    }
    Ok(())
}

/// Geometry for a freshly created content webview, so it lands correctly
/// before the first resize event.
pub fn content_rect(window: &Window<Wry>) -> (tauri::LogicalPosition<f64>, tauri::LogicalSize<f64>) {
    let scale = window.scale_factor().unwrap_or(1.0);
    let offset = titlebar_offset(window);
    let size = window.inner_size().map(|s| s.to_logical::<f64>(scale)).unwrap_or_default();
    let content_top = offset + CHROME_HEIGHT;
    let height = (size.height - content_top).max(0.0);
    (tauri::LogicalPosition::new(0.0, content_top), tauri::LogicalSize::new(size.width, height))
}
