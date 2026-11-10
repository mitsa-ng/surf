use crate::tabs::TabId;

#[derive(Debug, thiserror::Error)]
pub enum SuspensionError {
    #[error("tab {0} has no backing webview")]
    NoWebview(TabId),
    #[error("platform cannot suspend webviews yet")]
    Unsupported,
}

/// Unloads a background tab's webview while keeping its session state
/// (URL, scroll offset, form data) so resuming looks identical to the user.
pub trait TabSuspender {
    fn suspend(&mut self, id: TabId) -> Result<(), SuspensionError>;
    fn resume(&mut self, id: TabId) -> Result<(), SuspensionError>;
    fn is_suspended(&self, id: TabId) -> bool;
}

/// M1 placeholder: wry does not expose WKWebView unloading yet, so
/// suspension is modelled but never performed.
pub struct NoopSuspender;

impl TabSuspender for NoopSuspender {
    fn suspend(&mut self, _id: TabId) -> Result<(), SuspensionError> {
        Err(SuspensionError::Unsupported)
    }
    fn resume(&mut self, _id: TabId) -> Result<(), SuspensionError> {
        Err(SuspensionError::Unsupported)
    }
    fn is_suspended(&self, _id: TabId) -> bool {
        false
    }
}
