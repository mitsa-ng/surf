use std::collections::HashMap;
use std::collections::HashSet;

use surf_core::tabs::TabId;
use url::Url;

use crate::planner::MatchPattern;
use crate::ExtensionId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionRequest {
    Api(String),
    /// A host pattern the extension wants programmatic access to.
    Host(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptDecision {
    Grant,
    Deny,
}

#[derive(Default)]
struct ExtGrants {
    api: HashSet<String>,
    /// Granted host patterns kept as normalised strings so an identical
    /// re-request never prompts twice.
    host_patterns: HashSet<String>,
    hosts: Vec<MatchPattern>,
}

#[derive(Default)]
pub struct MemoryGrantStore {
    grants: HashMap<ExtensionId, ExtGrants>,
}

impl MemoryGrantStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn grant_api(&mut self, ext: &ExtensionId, api: &str) {
        self.grants.entry(ext.clone()).or_default().api.insert(api.to_owned());
    }

    pub fn grant_host(&mut self, ext: &ExtensionId, pattern: &str) -> Result<(), crate::planner::PatternError> {
        let p = MatchPattern::parse(pattern)?;
        let entry = self.grants.entry(ext.clone()).or_default();
        entry.hosts.push(p);
        entry.host_patterns.insert(pattern.to_owned());
        Ok(())
    }

    pub fn has_api(&self, ext: &ExtensionId, api: &str) -> bool {
        self.grants.get(ext).map(|g| g.api.contains(api)).unwrap_or(false)
    }

    pub fn has_host(&self, ext: &ExtensionId, url: &Url) -> bool {
        self.grants.get(ext).map(|g| g.hosts.iter().any(|p| p.matches(url))).unwrap_or(false)
    }

    pub fn has_tab_access(&self, ext: &ExtensionId, _tab: &TabId) -> bool {
        // Tab-scoped access requires the "tabs" api permission in M1.
        self.has_api(ext, "tabs")
    }
}

pub struct PermissionManager {
    store: MemoryGrantStore,
    prompt: Box<dyn Fn(&ExtensionId, &PermissionRequest) -> PromptDecision + Send + Sync>,
}

impl PermissionManager {
    pub fn new() -> Self {
        Self::with_prompt(|_, _| PromptDecision::Deny)
    }

    pub fn with_prompt<F>(prompt: F) -> Self
    where
        F: Fn(&ExtensionId, &PermissionRequest) -> PromptDecision + Send + Sync + 'static,
    {
        Self { store: MemoryGrantStore::new(), prompt: Box::new(prompt) }
    }

    /// Check the grant store first; on miss, ask the prompt and record
    /// the answer so the user is never asked twice for the same request.
    pub fn ensure(&mut self, ext: &ExtensionId, request: &PermissionRequest) -> bool {
        match request {
            PermissionRequest::Api(api) => {
                if self.store.has_api(ext, api) {
                    return true;
                }
                if (self.prompt)(ext, request) == PromptDecision::Grant {
                    self.store.grant_api(ext, api);
                    true
                } else {
                    false
                }
            }
            PermissionRequest::Host(pattern) => {
                if self.store.has_host_pattern(ext, pattern) {
                    return true;
                }
                if (self.prompt)(ext, request) == PromptDecision::Grant {
                    self.store.grant_host(ext, pattern).is_ok()
                } else {
                    false
                }
            }
        }
    }

    pub fn store(&self) -> &MemoryGrantStore {
        &self.store
    }

    pub fn store_mut(&mut self) -> &mut MemoryGrantStore {
        &mut self.store
    }
}

impl Default for PermissionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryGrantStore {
    pub fn has_host_pattern(&self, ext: &ExtensionId, pattern: &str) -> bool {
        self.grants.get(ext).map(|g| g.host_patterns.contains(pattern)).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_deny_by_default() {
        let mut pm = PermissionManager::new();
        let ext = ExtensionId("e".into());
        assert!(!pm.ensure(&ext, &PermissionRequest::Api("tabs".into())));
        assert!(!pm.store().has_api(&ext, "tabs"));
    }

    #[test]
    fn prompt_grant_is_remembered() {
        let mut pm = PermissionManager::with_prompt(|_, _| PromptDecision::Grant);
        let ext = ExtensionId("e".into());
        assert!(pm.ensure(&ext, &PermissionRequest::Api("storage".into())));
        // Second call must not re-prompt: grant store answers directly.
        assert!(pm.store().has_api(&ext, "storage"));
    }

    #[test]
    fn host_grant_matches_urls() {
        let mut store = MemoryGrantStore::new();
        let ext = ExtensionId("e".into());
        store.grant_host(&ext, "https://*.example.com/*").unwrap();
        assert!(store.has_host(&ext, &Url::parse("https://a.example.com/x").unwrap()));
        assert!(!store.has_host(&ext, &Url::parse("https://other.com/").unwrap()));
    }
}
