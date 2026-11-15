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

/// Mobile chrome geometry, measured by the UI (`offsetHeight` and
/// `innerWidth/Height` times `devicePixelRatio`) and reported through
/// `chrome_metrics_set`. Tauri reports logical==physical on Android
/// (scale 1.0) while the webviews render CSS at their own density, and
/// tao keeps the pre-inset window size when system bars or the IME
/// shrink the decor frame — so both the bar reserve and the content
/// area's extent must come measured, not computed.
#[tauri::command]
pub fn chrome_metrics_set(
    app: AppHandle,
    bottom_bar_px: u32,
    viewport_w_px: Option<u32>,
    viewport_h_px: Option<u32>,
) -> Result<(), String> {
    #[cfg(mobile)]
    {
        let state = app.state::<BrowserState>();
        state.bottom_bar_px.store(bottom_bar_px, Ordering::Relaxed);
        if let Some(w) = viewport_w_px {
            state.chrome_viewport_w_px.store(w, Ordering::Relaxed);
        }
        if let Some(h) = viewport_h_px {
            state.chrome_viewport_h_px.store(h, Ordering::Relaxed);
        }
        relayout(&app).map_err(|e| e.to_string())
    }
    #[cfg(desktop)]
    {
        let _ = (app, bottom_bar_px, viewport_w_px, viewport_h_px);
        Ok(())
    }
}

/// Total chrome height: toolbar stack plus the bookmarks bar when it is
/// visible. The UI header must render exactly this height.
#[cfg(desktop)]
pub fn chrome_height(state: &BrowserState) -> f64 {
    let bar = if cfg!(desktop) && state.bookmarks_bar.load(Ordering::Relaxed) {
        BOOKMARKS_BAR_HEIGHT_LOGICAL
    } else {
        0.0
    };
    CHROME_HEIGHT_LOGICAL + bar
}

pub fn relayout(app: &AppHandle) -> Result<(), tauri::Error> {
    let Some(window) = app.get_window(state::MAIN_LABEL) else {
        return Ok(());
    };
    #[cfg(mobile)]
    {
        // The ipc thread holds wry's IPC mutex for the whole command, and
        // this pass needs window getters (inner_size) that round-trip to
        // the tao loop — the same loop a concurrent webview create needs
        // that IPC mutex on. Running the pass on the loop thread breaks
        // the cycle; send_user_message executes the task inline when
        // already there, so main-thread callers stay synchronous.
        let window_for_task = window.clone();
        return window.run_on_main_thread(move || {
            let _ = relayout_window(&window_for_task);
        });
    }
    #[cfg(desktop)]
    relayout_window(&window)
}

pub fn relayout_window(window: &Window<Wry>) -> Result<(), tauri::Error> {
    let app = window.app_handle();
    let scale = window.scale_factor().unwrap_or(1.0);
    let size = window.inner_size()?;
    #[cfg(desktop)]
    let logical_size = size.to_logical::<f64>(scale);
    let state = app.state::<BrowserState>();

    // Desktop has a separate chrome strip whose height the overlays
    // expand; mobile renders chrome inside the full-screen main webview,
    // so only content geometry is managed here.
    #[cfg(desktop)]
    let expanded = state.panel_open.load(Ordering::Relaxed)
        || state.overlay_open.load(Ordering::Relaxed);
    #[cfg(desktop)]
    let (content_top, chrome) = {
        let chrome_h = if expanded { logical_size.height } else { chrome_height(&state) };
        let chrome = app.get_webview(CHROME_LABEL);
        if let Some(chrome) = &chrome {
            chrome.set_position(tauri::LogicalPosition::new(0.0, 0.0))?;
            chrome.set_size(tauri::LogicalSize::new(logical_size.width, chrome_h))?;
        }
        (chrome_height(&state), chrome)
    };
    #[cfg(mobile)]
    let content_top = 0.0;
    #[cfg(mobile)]
    let _ = scale;
    // tao's window size predates the decor insets (system bars, IME):
    // when the UI has reported the chrome webview's live physical
    // viewport, that is the truth for content geometry.
    #[cfg(mobile)]
    let (content_w, window_h) = {
        let vw = state.chrome_viewport_w_px.load(Ordering::Relaxed) as f64;
        let vh = state.chrome_viewport_h_px.load(Ordering::Relaxed) as f64;
        (
            if vw > 0.0 { vw } else { size.width as f64 },
            if vh > 0.0 { vh } else { size.height as f64 },
        )
    };

    let content_height = {
        #[cfg(desktop)]
        { (logical_size.height - content_top).max(0.0) }
        // Tauri reports logical==physical on Android (scale 1.0) while
        // the webviews render CSS at their own density, so the reserve
        // is the UI-reported physical bar height, not a logical constant.
        #[cfg(mobile)]
        {
            let bar = state.bottom_bar_px.load(Ordering::Relaxed) as f64;
            (window_h - bar).max(0.0)
        }
    };
    let ids: Vec<TabId> = state::lock(&state.webviews).keys().copied().collect();
    let split = *state::lock(&state.split);
    // The measured chrome viewport governs width on mobile (see above);
    // desktop keeps the tao logical width.
    #[cfg(mobile)]
    let layout_w = content_w;
    #[cfg(desktop)]
    let layout_w = logical_size.width;
    for id in ids {
        let Some(view) = state::content_webview(app, id) else { continue };
        match split {
            // Split view: the two members share the content area; every
            // other webview hides.
            Some((left, _)) if id == left => {
                view.raw().set_position(tauri::LogicalPosition::new(0.0, content_top))?;
                view.raw().set_size(tauri::LogicalSize::new(layout_w / 2.0, content_height))?;
                let _ = view.show();
            }
            Some((_, right)) if id == right => {
                view.raw().set_position(tauri::LogicalPosition::new(layout_w / 2.0, content_top))?;
                let half = layout_w - (layout_w / 2.0);
                view.raw().set_size(tauri::LogicalSize::new(half, content_height))?;
                let _ = view.show();
            }
            Some(_) => {
                let _ = view.hide();
            }
            None => {
                view.raw().set_position(tauri::LogicalPosition::new(0.0, content_top))?;
                view.raw().set_size(tauri::LogicalSize::new(layout_w, content_height))?;
            }
        }
    }
    #[cfg(desktop)]
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
/// Mobile overlays (tab sheet, panels) render inside the full-screen
/// main webview, which native content webviews stack above. Opening one
/// must lift the main webview to the front; closing hands the front back
/// to the active tab's webview.
#[cfg(mobile)]
pub fn raise_main_chrome(app: &AppHandle, open: bool) {
    if open {
        if let Some(webview) = app.get_webview(state::MAIN_LABEL) {
            let _ = webview.show();
        }
    } else {
        let state = app.state::<BrowserState>();
        let active = state::lock(&state.model).active_id();
        drop(state);
        if let Some(id) = active {
            if let Some(view) = state::content_webview(app, id) {
                let _ = view.show();
            }
        }
    }
}

/// Mobile: any path that shows a content webview while an overlay is
/// open (closing or opening a tab from the sheet) buries the main
/// webview again — the sheet's rows become untouchable. Callers that
/// touch the front view re-lift the chrome when an overlay is up.
#[cfg(mobile)]
pub fn restore_overlay_front(app: &AppHandle) {
    let state = app.state::<BrowserState>();
    let overlay = state.overlay_open.load(Ordering::Relaxed)
        || state.panel_open.load(Ordering::Relaxed);
    drop(state);
    if overlay {
        raise_main_chrome(app, true);
    }
}

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
        raise_main_chrome(&app, open);
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
        raise_main_chrome(&app, open);
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
/// Both tabs must exist and differ; the mobile layout shares the content
/// area the same way (half-width panes above the bottom bar).
#[tauri::command]
pub fn split_set(app: AppHandle, left: u64, right: u64) -> Result<(), String> {
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

/// Leave split view.
#[tauri::command]
pub fn split_clear(app: AppHandle) -> Result<(), String> {
    let state = app.state::<BrowserState>();
    let had = state::lock(&state.split).take().is_some();
    if had {
        crate::manager::persist_session(&app);
        return relayout(&app).map_err(|error| error.to_string());
    }
    Ok(())
}

/// Geometry for a freshly created content webview, so it lands correctly
/// before the first resize event.
pub fn content_rect(
    window: &Window<Wry>,
) -> (tauri::LogicalPosition<f64>, tauri::LogicalSize<f64>) {
    let scale = window.scale_factor().unwrap_or(1.0);
    let size = window.inner_size().unwrap_or_default();
    let logical_size = size.to_logical::<f64>(scale);
    let state = window.app_handle().state::<BrowserState>();
    #[cfg(desktop)]
    let content_top = chrome_height(&state);
    #[cfg(mobile)]
    let content_top = 0.0;
    #[cfg(mobile)]
    let _ = &logical_size;
    #[cfg(desktop)]
    let height = (logical_size.height - content_top).max(0.0);
    #[cfg(mobile)]
    let height = {
        let bar = state.bottom_bar_px.load(Ordering::Relaxed) as f64;
        (size.height as f64 - bar).max(0.0)
    };
    (
        tauri::LogicalPosition::new(0.0, content_top),
        tauri::LogicalSize::new(logical_size.width, height),
    )
}
