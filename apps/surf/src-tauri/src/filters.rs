use surf_ext_host::rules::FilterCapabilities;

/// Layer-3 request-filter boundary. Adapters translate engine decisions
/// into their platform's interception mechanism. `install` wiring lands
/// with the M2 content-script bridge, hence the allow.
pub trait RequestFilterAdapter: Send + Sync {
    fn capabilities(&self) -> FilterCapabilities;
    #[allow(dead_code)]
    fn install(&self, rules: &[String]) -> Result<(), FilterInstallError>;
}

#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum FilterInstallError {
    #[error("adapter is not implemented for this platform in M1: {0}")]
    NotImplemented(&'static str),
}

pub struct AppleContentRuleListAdapter;

impl RequestFilterAdapter for AppleContentRuleListAdapter {
    fn capabilities(&self) -> FilterCapabilities {
        FilterCapabilities::APPLE
    }

    fn install(&self, _rules: &[String]) -> Result<(), FilterInstallError> {
        Err(FilterInstallError::NotImplemented("WKContentRuleList compilation lands in M2"))
    }
}

pub struct WebView2Adapter;

impl RequestFilterAdapter for WebView2Adapter {
    fn capabilities(&self) -> FilterCapabilities {
        FilterCapabilities::WEBVIEW2
    }

    fn install(&self, _rules: &[String]) -> Result<(), FilterInstallError> {
        Err(FilterInstallError::NotImplemented("WebResourceRequested hook lands with the Windows target"))
    }
}

pub struct AndroidInterceptAdapter;

impl RequestFilterAdapter for AndroidInterceptAdapter {
    fn capabilities(&self) -> FilterCapabilities {
        FilterCapabilities::ANDROID
    }

    fn install(&self, _rules: &[String]) -> Result<(), FilterInstallError> {
        Err(FilterInstallError::NotImplemented("shouldInterceptRequest bridge lands with the Android target"))
    }
}

pub struct WebKitGtkAdapter;

impl RequestFilterAdapter for WebKitGtkAdapter {
    fn capabilities(&self) -> FilterCapabilities {
        FilterCapabilities::WEBKITGTK
    }

    fn install(&self, _rules: &[String]) -> Result<(), FilterInstallError> {
        Err(FilterInstallError::NotImplemented("WebKitUserContentFilter lands with the Linux target"))
    }
}

pub fn platform_adapter() -> Box<dyn RequestFilterAdapter> {
    #[cfg(target_os = "macos")]
    return Box::new(AppleContentRuleListAdapter);
    #[cfg(target_os = "windows")]
    return Box::new(WebView2Adapter);
    #[cfg(target_os = "android")]
    return Box::new(AndroidInterceptAdapter);
    #[cfg(all(target_os = "linux", not(target_os = "android")))]
    return Box::new(WebKitGtkAdapter);
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "android", target_os = "linux")))]
    {
        compile_error!("Surf has no request-filter adapter for this platform");
    }
}
