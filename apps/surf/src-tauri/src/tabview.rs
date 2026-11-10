use url::Url;

/// Layer-3 boundary from the architecture diagram: one TabView per tab,
/// implemented by the platform's webview. The manager never touches
/// platform webview types directly.
pub trait TabView {
    fn load(&self, url: &Url) -> Result<(), TabViewError>;
    fn eval(&self, js: &str) -> Result<(), TabViewError>;
    fn add_init_script(&self, js: &str) -> Result<(), TabViewError>;
    fn set_visible(&self, visible: bool) -> Result<(), TabViewError>;
}

#[derive(Debug, thiserror::Error)]
pub enum TabViewError {
    #[error("webview unavailable: {0}")]
    Unavailable(String),
}

pub struct TauriTabView(pub tauri::Webview<tauri::Wry>);

impl TabView for TauriTabView {
    fn load(&self, url: &Url) -> Result<(), TabViewError> {
        self.0
            .eval(&format!("location.replace({:?})", url.as_str()))
            .map_err(|e| TabViewError::Unavailable(e.to_string()))
    }

    fn eval(&self, js: &str) -> Result<(), TabViewError> {
        self.0.eval(js).map_err(|e| TabViewError::Unavailable(e.to_string()))
    }

    fn add_init_script(&self, _js: &str) -> Result<(), TabViewError> {
        // wry only supports init scripts at webview construction time;
        // the injection planner collects them before attach in M2.
        Err(TabViewError::Unavailable("init scripts are attach-time only; pre-plan them (M2)".into()))
    }

    fn set_visible(&self, visible: bool) -> Result<(), TabViewError> {
        let result = if visible { self.0.show() } else { self.0.hide() };
        result.map_err(|e| TabViewError::Unavailable(e.to_string()))
    }
}
