use url::Url;

/// Layer-3 boundary from the architecture diagram: one TabView per tab,
/// implemented by the platform's webview. Surface operations only;
/// geometry and lifecycle (show/hide/close) stay with the manager.
pub trait TabView {
    fn load(&self, url: &Url) -> Result<(), TabViewError>;
    fn eval(&self, js: &str) -> Result<(), TabViewError>;
    /// Used by the M2 injection planner before webview attach.
    #[allow(dead_code)]
    fn add_init_script(&self, js: &str) -> Result<(), TabViewError>;
}

#[derive(Debug, thiserror::Error)]
pub enum TabViewError {
    #[error("webview unavailable: {0}")]
    Unavailable(String),
}

pub struct TauriTabView(pub tauri::Webview<tauri::Wry>);

impl TauriTabView {
    pub fn raw(&self) -> &tauri::Webview<tauri::Wry> {
        &self.0
    }

    pub fn show(&self) -> Result<(), TabViewError> {
        self.0.show().map_err(|e| TabViewError::Unavailable(e.to_string()))
    }

    pub fn hide(&self) -> Result<(), TabViewError> {
        self.0.hide().map_err(|e| TabViewError::Unavailable(e.to_string()))
    }

    pub fn close(&self) -> Result<(), TabViewError> {
        self.0.close().map_err(|e| TabViewError::Unavailable(e.to_string()))
    }
}

impl TabView for TauriTabView {
    fn load(&self, url: &Url) -> Result<(), TabViewError> {
        self.0
            .eval(format!("location.replace({:?})", url.as_str()))
            .map_err(|e| TabViewError::Unavailable(e.to_string()))
    }

    fn eval(&self, js: &str) -> Result<(), TabViewError> {
        self.0.eval(js).map_err(|e| TabViewError::Unavailable(e.to_string()))
    }

    fn add_init_script(&self, _js: &str) -> Result<(), TabViewError> {
        // wry only accepts init scripts at construction time; the
        // injection planner collects them before attach in M2.
        Err(TabViewError::Unavailable("init scripts are attach-time only; pre-plan them (M2)".into()))
    }
}
