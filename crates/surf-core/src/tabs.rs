use std::collections::HashMap;
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
    /// Private tab: no history recording and no session persistence.
    /// Storage isolation is per the platform webview profile; see NOTES.
    #[serde(default)]
    pub incognito: bool,
    /// Pinned tab: kept in the leftmost group and protected from the
    /// tab-strip close button.
    #[serde(default)]
    pub pinned: bool,
    /// Webview history journal state, mirrored to the UI for the disabled
    /// back/forward buttons. Restored tabs start at false: the session
    /// restore reloads one URL, so there is genuinely nothing behind it.
    #[serde(default)]
    pub can_back: bool,
    #[serde(default)]
    pub can_forward: bool,
}

/// Rust-side source of truth for the tab list. The UI layer never derives
/// this state itself; it mirrors the snapshots this model emits.
#[derive(Debug, Default)]
pub struct TabModel {
    tabs: Vec<TabState>,
    active: Option<TabId>,
    next_id: u64,
    /// Per-tab navigation journal. Tauri does not expose the webview's
    /// own back/forward availability, so committed navigations are
    /// journalled here and the index tracks the optimistic back/forward
    /// steps the UI commands take.
    history: HashMap<TabId, TabJournal>,
}

#[derive(Debug, Default)]
struct TabJournal {
    entries: Vec<Url>,
    index: usize,
}

impl TabJournal {
    /// Fold a committed navigation into the journal: same entry means a
    /// reload or the confirmation of a step we already counted; the entry
    /// after the cursor means forward; anything else is a new navigation
    /// that truncates the forward stack, like a webview would.
    fn record(&mut self, url: Url) -> (bool, bool) {
        if self.entries.get(self.index) == Some(&url) {
            return self.flags();
        }
        if self.entries.get(self.index + 1) == Some(&url) {
            self.index += 1;
            return self.flags();
        }
        self.entries.truncate(self.index + 1);
        self.entries.push(url);
        self.index = self.entries.len() - 1;
        self.flags()
    }

    fn flags(&self) -> (bool, bool) {
        (self.index > 0, self.index + 1 < self.entries.len())
    }
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
        self.open_with(url, false)
    }

    /// Open a private (incognito) tab.
    pub fn open_private(&mut self, url: Option<Url>) -> TabId {
        self.open_with(url, true)
    }

    fn open_with(&mut self, url: Option<Url>, incognito: bool) -> TabId {
        self.next_id += 1;
        let id = TabId(self.next_id);
        let tab = TabState {
            id,
            url: url.clone(),
            title: None,
            loading: url.is_some(),
            incognito,
            pinned: false,
            can_back: false,
            can_forward: false,
        };
        self.tabs.push(tab);
        self.active = Some(id);
        id
    }

    /// Pin or unpin a tab. Pinned tabs always occupy the leftmost
    /// positions; pin moves the tab before the first unpinned one, unpin
    /// moves it after the last pinned one.
    pub fn set_pinned(&mut self, id: TabId, pinned: bool) -> bool {
        let Some(idx) = self.tabs.iter().position(|t| t.id == id) else {
            return false;
        };
        if self.tabs[idx].pinned == pinned {
            return true;
        }
        self.tabs[idx].pinned = pinned;
        let tab = self.tabs.remove(idx);
        let target = if pinned {
            self.tabs.iter().position(|t| !t.pinned).unwrap_or(self.tabs.len())
        } else {
            self.tabs.iter().rposition(|t| t.pinned).map_or(0, |i| i + 1)
        };
        self.tabs.insert(target, tab);
        true
    }

    pub fn close(&mut self, id: TabId) -> Option<ClosedTab> {
        let idx = self.tabs.iter().position(|t| t.id == id)?;
        self.tabs.remove(idx);
        self.history.remove(&id);
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

    /// Fold a committed navigation into the tab's history journal and
    /// refresh the mirrored back/forward flags. Returns whether any
    /// mirrored flag changed, so callers can skip redundant snapshots;
    /// `false` covers unknown tabs too.
    pub fn record_navigation(&mut self, id: TabId, url: Url) -> bool {
        let flags = {
            let journal = self.history.entry(id).or_default();
            journal.record(url)
        };
        self.apply_flags(id, flags).unwrap_or(false)
    }

    /// Step back through the journal (the UI command already decided to
    /// navigate; this moves the cursor optimistically and reports whether
    /// there was anywhere to go).
    pub fn history_back(&mut self, id: TabId) -> bool {
        let Some(journal) = self.history.get_mut(&id) else { return false };
        if journal.index == 0 {
            return false;
        }
        journal.index -= 1;
        let flags = journal.flags();
        self.apply_flags(id, flags);
        true
    }

    /// Step forward through the journal; see `history_back`.
    pub fn history_forward(&mut self, id: TabId) -> bool {
        let Some(journal) = self.history.get_mut(&id) else { return false };
        if journal.index + 1 >= journal.entries.len() {
            return false;
        }
        journal.index += 1;
        let flags = journal.flags();
        self.apply_flags(id, flags);
        true
    }

    /// Write the journal flags into the tab state; `Some(changed)` says
    /// whether the mirrored booleans moved, `None` means unknown tab.
    fn apply_flags(&mut self, id: TabId, (back, forward): (bool, bool)) -> Option<bool> {
        self.mut_tab(id).map(|t| {
            let changed = t.can_back != back || t.can_forward != forward;
            t.can_back = back;
            t.can_forward = forward;
            changed
        })
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

/// Wall-clock ms; shared by storage and sync payloads.
pub fn now_ms_helper() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn journal_tracks_back_forward_and_truncation() {
        let mut model = TabModel::new();
        let id = model.open(None);
        // Returns whether a mirrored flag changed.
        let nav = |m: &mut TabModel, u: &str| m.record_navigation(id, url(u));
        // Fresh webview loads its start page: nothing behind or ahead.
        assert!(!nav(&mut model, "surf://start"));
        assert!(!model.get(id).unwrap().can_back);
        assert!(!model.get(id).unwrap().can_forward);
        // Two committed navigations enable back.
        assert!(nav(&mut model, "https://a.example/"));
        assert!(model.get(id).unwrap().can_back);
        assert!(!model.get(id).unwrap().can_forward);
        assert!(!nav(&mut model, "https://b.example/"));
        // Optimistic back keeps back (the start page is still behind) and
        // re-enables forward.
        assert!(model.history_back(id));
        assert!(model.get(id).unwrap().can_back);
        assert!(model.get(id).unwrap().can_forward);
        // The confirming navigation for the stepped-to entry is a no-op.
        assert!(!nav(&mut model, "https://a.example/"));
        // Back to the root, then a brand-new navigation truncates forward.
        assert!(model.history_back(id));
        assert!(!model.get(id).unwrap().can_back);
        assert!(model.history_forward(id));
        assert!(nav(&mut model, "https://c.example/"));
        assert!(model.get(id).unwrap().can_back);
        assert!(!model.get(id).unwrap().can_forward);
        // Back at the journal root is refused.
        assert!(model.history_back(id));
        assert!(model.history_back(id));
        assert!(!model.history_back(id));
        // Closing the tab drops its journal.
        model.close(id);
        assert!(!model.history_forward(id));
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
    fn private_tab_carries_flag_and_defaults_off() {
        let mut m = TabModel::new();
        let normal = m.open(None);
        let private = m.open_private(Some(url("https://secret.example/")));
        assert!(!m.get(normal).unwrap().incognito);
        assert!(m.get(private).unwrap().incognito);
        // Serialized snapshots round-trip the flag (serde default keeps
        // old payloads readable).
        let json = serde_json::to_string(m.get(private).unwrap()).unwrap();
        assert!(json.contains("\"incognito\":true"));
    }

    #[test]
    fn pin_moves_to_left_group_and_unpin_appends_after_pinned() {
        let mut m = TabModel::new();
        let a = m.open(Some(url("https://a.example/")));
        let b = m.open(Some(url("https://b.example/")));
        let c = m.open(Some(url("https://c.example/")));

        assert!(m.set_pinned(c, true));
        // Pinning c moves it before the first unpinned tab (position 0).
        assert_eq!(m.list().iter().map(|t| t.id).collect::<Vec<_>>(), vec![c, a, b]);

        assert!(m.set_pinned(a, true));
        // Pinning a joins it to the pinned group: [c, a] then unpinned [b].
        assert_eq!(m.list().iter().map(|t| t.id).collect::<Vec<_>>(), vec![c, a, b]);

        assert!(m.set_pinned(a, false));
        // Unpinning a moves it right after the last pinned (c).
        assert_eq!(m.list().iter().map(|t| t.id).collect::<Vec<_>>(), vec![c, a, b]);
        assert!(m.get(c).unwrap().pinned);
        assert!(!m.get(a).unwrap().pinned);

        // Unknown id is a no-op.
        assert!(!m.set_pinned(TabId(999), true));
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
