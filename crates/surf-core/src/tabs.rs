use std::fmt;

use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TabId(pub u64);

impl fmt::Display for TabId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tab-{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabState {
    pub id: TabId,
    pub url: Option<Url>,
    pub title: Option<String>,
    pub loading: bool,
}

/// Rust-side source of truth for the tab list. The UI layer never derives
/// this state itself; it mirrors the snapshots this model emits.
#[derive(Debug, Default)]
pub struct TabModel {
    tabs: Vec<TabState>,
    active: Option<TabId>,
    next_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedTab {
    pub closed: TabId,
    pub new_active: Option<TabId>,
}

impl TabModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self, url: Option<Url>) -> TabId {
        self.next_id += 1;
        let id = TabId(self.next_id);
        let tab = TabState { id, url: url.clone(), title: None, loading: url.is_some() };
        self.tabs.push(tab);
        self.active = Some(id);
        id
    }

    pub fn close(&mut self, id: TabId) -> Option<ClosedTab> {
        let idx = self.tabs.iter().position(|t| t.id == id)?;
        self.tabs.remove(idx);
        if self.active == Some(id) {
            // Prefer the tab that slid into the closed slot, else the left neighbour.
            self.active = self.tabs.get(idx).map(|t| t.id).or_else(|| self.tabs.get(idx.saturating_sub(1)).map(|t| t.id));
        }
        Some(ClosedTab { closed: id, new_active: self.active })
    }

    pub fn activate(&mut self, id: TabId) -> bool {
        if self.tabs.iter().any(|t| t.id == id) {
            self.active = Some(id);
            true
        } else {
            false
        }
    }

    pub fn set_url(&mut self, id: TabId, url: Url) -> bool {
        self.mut_tab(id).map(|t| t.url = Some(url)).is_some()
    }

    pub fn set_title(&mut self, id: TabId, title: String) -> bool {
        self.mut_tab(id).map(|t| t.title = Some(title)).is_some()
    }

    pub fn set_loading(&mut self, id: TabId, loading: bool) -> bool {
        self.mut_tab(id).map(|t| t.loading = loading).is_some()
    }

    pub fn active_id(&self) -> Option<TabId> {
        self.active
    }

    pub fn active(&self) -> Option<&TabState> {
        self.active.and_then(|id| self.tabs.iter().find(|t| t.id == id))
    }

    pub fn get(&self, id: TabId) -> Option<&TabState> {
        self.tabs.iter().find(|t| t.id == id)
    }

    pub fn list(&self) -> &[TabState] {
        &self.tabs
    }

    fn mut_tab(&mut self, id: TabId) -> Option<&mut TabState> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn open_appends_and_activates() {
        let mut m = TabModel::new();
        let a = m.open(Some(url("https://a.example/")));
        let b = m.open(Some(url("https://b.example/")));
        assert_eq!(m.list().len(), 2);
        assert_eq!(m.active_id(), Some(b));
        assert_ne!(a, b);
    }

    #[test]
    fn close_active_prefers_right_neighbour() {
        let mut m = TabModel::new();
        let a = m.open(Some(url("https://a.example/")));
        let b = m.open(Some(url("https://b.example/")));
        let c = m.open(Some(url("https://c.example/")));
        m.activate(b);
        let out = m.close(b).unwrap();
        assert_eq!(out.new_active, Some(c));
        assert_eq!(m.list().len(), 2);
        assert_eq!(m.close(a).unwrap().new_active, Some(c));
        assert_eq!(m.close(c).unwrap().new_active, None);
    }

    #[test]
    fn close_last_tab_leaves_empty() {
        let mut m = TabModel::new();
        let a = m.open(None);
        assert_eq!(m.close(a).unwrap().new_active, None);
        assert!(m.list().is_empty());
        assert_eq!(m.active_id(), None);
    }

    #[test]
    fn close_inactive_keeps_active() {
        let mut m = TabModel::new();
        let a = m.open(Some(url("https://a.example/")));
        let b = m.open(Some(url("https://b.example/")));
        let out = m.close(a).unwrap();
        assert_eq!(out.new_active, Some(b));
        assert_eq!(m.active_id(), Some(b));
    }

    #[test]
    fn updates_ignore_unknown_ids() {
        let mut m = TabModel::new();
        let a = m.open(None);
        assert!(!m.set_url(TabId(999), url("https://x.example/")));
        assert!(m.get(TabId(999)).is_none());
        assert!(m.set_url(a, url("https://a.example/")));
        assert_eq!(m.get(a).unwrap().url, Some(url("https://a.example/")));
        assert!(!m.activate(TabId(999)));
    }
}
