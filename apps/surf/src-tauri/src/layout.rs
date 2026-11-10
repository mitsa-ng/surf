use surf_core::tabs::TabId;
use tauri::{AppHandle, Window, Wry};

use crate::state::{self, BrowserState};

/// Desktop tab geometry. Chrome strip on top, content fills the rest.
/// Mobile chrome lives at the bottom of the UI layout instead and has no
/// content webviews in M1.
pub const CHROME_HEIGHT: f64 = 80.0;

pub const CHROME_LABEL: &str = "chrome";

pub fn relayout(app: &AppHandle) -> Result<(), tauri::Error> {
    let Some(window) = app.get_window(state::MAIN_LABEL) else {
        return Ok(());
    };
    let size = window.inner_size()?;
    let scale = window.scale_factor();
    let width = size.width as f64 / scale;
    let height = size.height as f64 / scale;

    if let Some(chrome) = app.get_webview(CHROME_LABEL) {
        chrome.set_position(tauri::LogicalPosition::new(0.0, 0.0))?;
        chrome.set_size(tauri::LogicalSize::new(width, CHROME_HEIGHT))?;
    }

    let state = app.state::<BrowserState>();
    let ids: Vec<TabId> = state::lock(&state.webviews).keys().copied().collect();
    let content_height = (height - CHROME_HEIGHT).max(0.0);
    for id in ids {
        if let Some(wv) = state::content_webview(app, id) {
            wv.set_position(tauri::LogicalPosition::new(0.0, CHROME_HEIGHT))?;
            wv.set_size(tauri::LogicalSize::new(width, content_height))?;
        }
    }
    Ok(())
}

/// Geometry for a freshly created content webview, so it lands correctly
/// before the first resize event.
pub fn content_rect(window: &Window<Wry>) -> (tauri::LogicalPosition<f64>, tauri::LogicalSize<f64>) {
    let size = window.inner_size().map(|s| s.to_logical::<f64>(window.scale_factor())).unwrap_or_default();
    let height = (size.height - CHROME_HEIGHT).max(0.0);
    (tauri::LogicalPosition::new(0.0, CHROME_HEIGHT), tauri::LogicalSize::new(size.width, height))
}
