use std::collections::HashMap;
use std::sync::Arc;

use surf_core::tabs::TabId;

use crate::ExtensionId;

#[derive(Debug, Clone)]
pub struct ExtContext {
    pub ext: ExtensionId,
    pub tab: Option<TabId>,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("unknown module: {0}")]
    UnknownModule(String),
    #[error("unknown method {module}.{method}")]
    UnknownMethod { module: String, method: String },
    #[error("{module}.{method} is not implemented in this milestone")]
    NotImplemented { module: String, method: String },
    #[error("permission denied for {0}")]
    Denied(String),
}

pub trait ApiModule: Send + Sync {
    fn name(&self) -> &'static str;
    fn call(&self, ctx: &ExtContext, method: &str, payload: &serde_json::Value) -> Result<serde_json::Value, ApiError>;
}

/// Registry of namespaced APIs exposed to extension runtimes. M1 ships
/// the surface with NotImplemented errors so permission wiring and the
/// message path can be exercised end to end.
pub struct ApiHost {
    modules: HashMap<&'static str, Arc<dyn ApiModule>>,
}

impl Default for ApiHost {
    fn default() -> Self {
        Self::new()
    }
}

impl ApiHost {
    pub fn new() -> Self {
        let mut host = Self { modules: HashMap::new() };
        host.register(Arc::new(NotImplementedModule::new("storage")));
        host.register(Arc::new(NotImplementedModule::new("tabs")));
        host.register(Arc::new(NotImplementedModule::new("bookmarks")));
        host.register(Arc::new(NotImplementedModule::new("alarms")));
        host.register(Arc::new(NotImplementedModule::new("fetch")));
        host
    }

    pub fn register(&mut self, module: Arc<dyn ApiModule>) {
        self.modules.insert(module.name(), module);
    }

    pub fn call(&self, ctx: &ExtContext, module: &str, method: &str, payload: &serde_json::Value) -> Result<serde_json::Value, ApiError> {
        let m = self.modules.get(module).ok_or_else(|| ApiError::UnknownModule(module.to_owned()))?;
        m.call(ctx, method, payload)
    }
}

struct NotImplementedModule {
    name: &'static str,
}

impl NotImplementedModule {
    fn new(name: &'static str) -> Self {
        Self { name }
    }
}

impl ApiModule for NotImplementedModule {
    fn name(&self) -> &'static str {
        self.name
    }

    fn call(&self, _ctx: &ExtContext, method: &str, _payload: &serde_json::Value) -> Result<serde_json::Value, ApiError> {
        Err(ApiError::NotImplemented { module: self.name().to_owned(), method: method.to_owned() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_modules_report_not_implemented() {
        let host = ApiHost::new();
        let ctx = ExtContext { ext: ExtensionId("e".into()), tab: None };
        let err = host.call(&ctx, "storage", "local.get", &serde_json::json!({})).unwrap_err();
        assert!(matches!(err, ApiError::NotImplemented { .. }));
    }

    #[test]
    fn unknown_module_is_distinct() {
        let host = ApiHost::new();
        let ctx = ExtContext { ext: ExtensionId("e".into()), tab: None };
        let err = host.call(&ctx, "webrequest", "x", &serde_json::json!({})).unwrap_err();
        assert!(matches!(err, ApiError::UnknownModule(_)));
    }
}
