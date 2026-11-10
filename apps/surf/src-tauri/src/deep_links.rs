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
            let _ = handle.get_webview("chrome").and_then(|w| {
                w.emit_to("chrome", EVENT, url.to_string()).ok()
            });
            #[cfg(mobile)]
            {
                // Mobile has a single webview window; forward there too.
                let _ = handle
                    .get_webview_window(crate::state::MAIN_LABEL)
                    .and_then(|w| w.emit_to(crate::state::MAIN_LABEL, EVENT, url.to_string()).ok());
            }
        }
    });
}
