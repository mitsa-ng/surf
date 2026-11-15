use std::collections::HashSet;
use std::sync::Mutex;

use surf_core::tabs::TabId;
use surf_ext_host::rules::FilterCapabilities;
use tauri::{AppHandle, Manager, Wry};

use crate::manager::content_label;
use crate::state::{self, BrowserState};

/// Per-tab install bookkeeping: WKContentRuleList installs crash on a
/// controller whose page is not initialized yet (null proxy), so the
/// first install waits for the tab's first finished page load.
#[derive(Default)]
struct InstallState {
    loaded: HashSet<TabId>,
    pending: HashSet<TabId>,
}

fn with_state<T>(f: impl FnOnce(&mut InstallState) -> T) -> T {
    static STATE: Mutex<Option<InstallState>> = Mutex::new(None);
    let mut guard = STATE.lock().unwrap();
    f(guard.get_or_insert_with(InstallState::default))
}

/// Called when a tab finishes its first page load: the controller's
/// proxy exists from here on.
pub fn tab_loaded(app: &AppHandle, id: TabId) {
    let pending = with_state(|s| {
        s.loaded.insert(id);
        s.pending.remove(&id)
    });
    if pending {
        install_now(app, id);
    }
}

/// Layer-3 request-filter boundary. Adapters translate the shared filter
/// list into their platform's interception mechanism. Navigation-level
/// blocking is handled by `nav_filter` on every platform; this layer is
/// subresource blocking inside loaded pages.
pub trait RequestFilterAdapter: Send + Sync {
    fn capabilities(&self) -> FilterCapabilities;
    /// Compile `rules` and install them on the webview registered under
    /// `label`. Platforms whose interception hook wry does not expose
    /// return `FilterInstallError::NotImplemented`.
    fn install(&self, app: &AppHandle<Wry>, label: &str, rules: &[String]) -> Result<(), FilterInstallError>;
}

#[derive(Debug, thiserror::Error)]
pub enum FilterInstallError {
    /// Constructed only on platforms whose adapter is still a stub.
    #[allow(dead_code)]
    #[error("adapter is not implemented for this platform: {0}")]
    NotImplemented(&'static str),
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub struct AppleContentRuleListAdapter;

#[cfg(any(target_os = "macos", target_os = "ios"))]
impl RequestFilterAdapter for AppleContentRuleListAdapter {
    fn capabilities(&self) -> FilterCapabilities {
        FilterCapabilities::APPLE
    }

    fn install(&self, app: &AppHandle<Wry>, label: &str, rules: &[String]) -> Result<(), FilterInstallError> {
        let json = surf_ext_host::rules::content_blocker::to_content_blocker_json(rules.iter().map(String::as_str));
        crate::wk_rules::install(app, label, &json);
        Ok(())
    }
}

#[cfg(target_os = "windows")]
pub struct WebView2Adapter;

#[cfg(target_os = "windows")]
impl RequestFilterAdapter for WebView2Adapter {
    fn capabilities(&self) -> FilterCapabilities {
        FilterCapabilities::WEBVIEW2
    }

    fn install(&self, _app: &AppHandle<Wry>, _label: &str, _rules: &[String]) -> Result<(), FilterInstallError> {
        Err(FilterInstallError::NotImplemented("wry does not expose WebResourceRequested to Rust"))
    }
}

#[cfg(target_os = "android")]
pub struct AndroidInterceptAdapter;

/// Raw-URL subresource verdict: unparseable URLs fail open, matching
/// the navigation filter, so a malformed request never wedges a page.
/// Outside Android this is only exercised by tests.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn subresource_allowed(nav: &crate::nav_filter::NavFilter, url: &str) -> bool {
    match url::Url::parse(url) {
        Ok(u) => nav.allows(&u),
        Err(_) => true,
    }
}

#[cfg(target_os = "android")]
impl RequestFilterAdapter for AndroidInterceptAdapter {
    fn capabilities(&self) -> FilterCapabilities {
        FilterCapabilities::ANDROID
    }

    fn install(&self, app: &AppHandle<Wry>, _label: &str, _rules: &[String]) -> Result<(), FilterInstallError> {
        // shouldInterceptRequest is process-global in wry's Android
        // binding, so one predicate covers every tab. It consults the
        // same engine thread nav_filter runs — reloading rules restarts
        // that thread, so the closure never goes stale — and with no
        // filters configured the engine is absent and allows() fails
        // open without a roundtrip. URL-only verdicts: wry hands over
        // the request URL, not the page URL, so $domain= rules behave
        // as if unspecified.
        let handle = app.clone();
        // Blocker contract is "true = block", the inverse of allows().
        tauri::wry::prelude::set_request_blocker(Some(Box::new(move |url| {
            let state = handle.state::<BrowserState>();
            !subresource_allowed(&state.nav_filter, url)
        })));
        Ok(())
    }
}

#[cfg(all(target_os = "linux", not(target_os = "android")))]
pub struct WebKitGtkAdapter;

#[cfg(all(target_os = "linux", not(target_os = "android")))]
impl RequestFilterAdapter for WebKitGtkAdapter {
    fn capabilities(&self) -> FilterCapabilities {
        FilterCapabilities::WEBKITGTK
    }

    fn install(&self, _app: &AppHandle<Wry>, _label: &str, _rules: &[String]) -> Result<(), FilterInstallError> {
        Err(FilterInstallError::NotImplemented("WebKitUserContentFilter lands with the Linux target"))
    }
}

pub fn platform_adapter() -> Box<dyn RequestFilterAdapter> {
    // Apple platforms share WKContentRuleList: macOS, iOS, and Apple-tvOS-style builds.
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    return Box::new(AppleContentRuleListAdapter);
    #[cfg(target_os = "windows")]
    return Box::new(WebView2Adapter);
    #[cfg(target_os = "android")]
    return Box::new(AndroidInterceptAdapter);
    #[cfg(all(target_os = "linux", not(target_os = "android")))]
    return Box::new(WebKitGtkAdapter);
    #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "windows", target_os = "android", target_os = "linux")))]
    {
        compile_error!("Surf has no request-filter adapter for this platform");
    }
}

/// Install the persisted filter list on one content webview now. The
/// navigation filter stays authoritative: subresource install failures
/// are logged, not propagated.
fn install_now(app: &AppHandle, id: TabId) {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        let filters = {
            let state = app.state::<BrowserState>();
            let db = state::lock(&state.db);
            crate::nav_filter::load_filters(&db)
        };
        let label = content_label(id);
        if let Err(e) = AppleContentRuleListAdapter.install(app, &label, &filters) {
            eprintln!("[filters] {label}: {e}");
        }
    }
    // The Android blocker is process-global; installing per tab just
    // means the first tab to finish a load arms it.
    #[cfg(target_os = "android")]
    {
        let filters = {
            let state = app.state::<BrowserState>();
            let db = state::lock(&state.db);
            crate::nav_filter::load_filters(&db)
        };
        let label = content_label(id);
        if let Err(e) = AndroidInterceptAdapter.install(app, &label, &filters) {
            eprintln!("[filters] {label}: {e}");
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
    let _ = (app, id);
}

/// Apply the persisted filter list to one content webview: immediately
/// if the tab has loaded once, otherwise deferred to its first load.
pub fn apply_filters(app: &AppHandle, id: TabId) {
    let ready = with_state(|s| {
        if s.loaded.contains(&id) {
            true
        } else {
            s.pending.insert(id);
            false
        }
    });
    if ready {
        install_now(app, id);
    }
}

/// Re-apply the persisted filter list to every tab (desktop child
/// webviews and mobile tab windows share the label scheme).
pub fn refresh_all(app: &AppHandle) {
    let ids: Vec<TabId> = {
        let state = app.state::<BrowserState>();
        let model = state::lock(&state.model);
        model.list().iter().map(|t| t.id).collect()
    };
    for id in ids {
        apply_filters(app, id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_blocker_verdict_blocks_and_fails_open() {
        let nav = crate::nav_filter::NavFilter::default();
        // No engine configured: everything allowed, no roundtrip.
        assert!(subresource_allowed(&nav, "https://ads.example/x.js"));
        nav.start(vec!["||ads.example^".into()]);
        std::thread::sleep(std::time::Duration::from_millis(150));
        assert!(!subresource_allowed(&nav, "https://ads.example/tracker.js"));
        assert!(subresource_allowed(&nav, "https://ok.example/app.js"));
        // Malformed URL: fail open like the navigation filter.
        assert!(subresource_allowed(&nav, "not a url"));
    }
}
