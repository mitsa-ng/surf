//! surf:// deep links. The shell receives them through the deep-link
//! plugin and forwards the raw URL to the chrome UI, which maps known
//! actions (surf://home, surf://settings, surf://open?url=…) onto normal
//! navigation. Scheme registration activates for bundled apps via the
//! CFBundleURLTypes entries in the generated plists.

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_deep_link::DeepLinkExt;

pub const EVENT: &str = "deeplink://open";

pub fn wire(app: &AppHandle) {
    let handle = app.clone();
    app.deep_link().on_open_url(move |event| {
        for url in event.urls() {
            forward(&handle, url.as_ref());
            // Cold start: the runtime event can fire before the chrome UI
            // mounts its listener, which loses the URL; queue it too so
            // the UI takes it once ready. The UI's single take at mount
            // cannot double-apply it.
            let state = handle.state::<crate::state::BrowserState>();
            let mut queued = crate::state::lock(&state.launch_deeplink);
            if queued.is_none() {
                *queued = Some(url.to_string());
            }
        }
    });
    // Launch URLs that predate wiring land here instead.
    if let Some(first) = app.deep_link().get_current().ok().flatten().and_then(|urls| urls.first().cloned()) {
        let state = app.state::<crate::state::BrowserState>();
        let mut queued = crate::state::lock(&state.launch_deeplink);
        if queued.is_none() {
            *queued = Some(first.to_string());
        }
    }
}

fn forward(app: &AppHandle, url: &str) {
    let _ = app.get_webview("chrome").and_then(|w| w.emit_to("chrome", EVENT, url.to_string()).ok());
    #[cfg(mobile)]
    {
        // Mobile has a single webview window; forward there too.
        let _ = app
            .get_webview_window(crate::state::MAIN_LABEL)
            .and_then(|w| w.emit_to(crate::state::MAIN_LABEL, EVENT, url.to_string()).ok());
    }
}
