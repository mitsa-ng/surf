use std::path::Path;

use rusqlite::{params, Connection};
use url::Url;

use crate::tabs::{TabId, TabState};

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

pub type Result<T> = std::result::Result<T, StorageError>;

const MIGRATIONS: &[&str] = &[
    // v1: session tabs, history, bookmarks
    "CREATE TABLE tabs (
        id INTEGER PRIMARY KEY,
        position INTEGER NOT NULL,
        url TEXT,
        title TEXT
    );
    CREATE TABLE history (
        url TEXT NOT NULL,
        title TEXT,
        visited_at INTEGER NOT NULL
    );
    CREATE INDEX idx_history_visited_at ON history(visited_at DESC);
    CREATE TABLE bookmarks (
        url TEXT PRIMARY KEY,
        title TEXT,
        created_at INTEGER NOT NULL
    );",
    // v2: per-extension key/value storage
    "CREATE TABLE ext_kv (
        ext TEXT NOT NULL,
        key TEXT NOT NULL,
        value TEXT NOT NULL,
        PRIMARY KEY (ext, key)
    );",
];

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = if path.as_ref().as_os_str() == ":memory:" {
            Connection::open_in_memory()?
        } else {
            Connection::open(path)?
        };
        let mut db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&mut self) -> Result<()> {
        let version: i64 = self.conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        for (i, sql) in MIGRATIONS.iter().enumerate() {
            let target = (i + 1) as i64;
            if version < target {
                self.conn.execute_batch(sql)?;
                self.conn.pragma_update(None, "user_version", target)?;
            }
        }
        Ok(())
    }

    pub fn save_tab(&self, position: usize, tab: &TabState) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO tabs (id, position, url, title) VALUES (?1, ?2, ?3, ?4)",
            params![tab.id.0 as i64, position as i64, tab.url.as_ref().map(|u| u.as_str()), tab.title],
        )?;
        Ok(())
    }

    pub fn delete_tab(&self, id: TabId) -> Result<()> {
        self.conn.execute("DELETE FROM tabs WHERE id = ?1", params![id.0 as i64])?;
        Ok(())
    }

    pub fn clear_tabs(&self) -> Result<()> {
        self.conn.execute("DELETE FROM tabs", [])?;
        Ok(())
    }

    /// Restore the persisted session, ordered by tab position.
    pub fn load_tabs(&self) -> Result<Vec<TabState>> {
        let mut stmt = self.conn.prepare("SELECT id, url, title FROM tabs ORDER BY position")?;
        let rows = stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let url: Option<String> = row.get(1)?;
            let title: Option<String> = row.get(2)?;
            Ok(TabState {
                id: TabId(id as u64),
                url: url.and_then(|u| Url::parse(&u).ok()),
                title,
                loading: false,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn record_visit(&self, url: &Url, title: Option<&str>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO history (url, title, visited_at) VALUES (?1, ?2, ?3)",
            params![url.as_str(), title, now_ms()],
        )?;
        Ok(())
    }

    /// Delete every history row; returns the number of rows removed.
    pub fn clear_history(&self) -> Result<usize> {
        Ok(self.conn.execute("DELETE FROM history", [])?)
    }

    pub fn history_recent(&self, limit: usize) -> Result<Vec<(Url, Option<String>, i64)>> {
        let mut stmt =
            self.conn.prepare("SELECT url, title, visited_at FROM history ORDER BY visited_at DESC LIMIT ?1")?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            let url: String = row.get(0)?;
            let title: Option<String> = row.get(1)?;
            let visited_at: i64 = row.get(2)?;
            Ok((url, title, visited_at))
        })?;
        Ok(rows
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .filter_map(|(u, t, ts)| Url::parse(&u).ok().map(|u| (u, t, ts)))
            .collect())
    }

    pub fn add_bookmark(&self, url: &Url, title: Option<&str>) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO bookmarks (url, title, created_at) VALUES (?1, ?2, ?3)",
            params![url.as_str(), title, now_ms()],
        )?;
        Ok(())
    }

    pub fn remove_bookmark(&self, url: &Url) -> Result<()> {
        self.conn.execute("DELETE FROM bookmarks WHERE url = ?1", params![url.as_str()])?;
        Ok(())
    }

    pub fn bookmarks(&self) -> Result<Vec<(Url, Option<String>)>> {
        let mut stmt = self.conn.prepare("SELECT url, title FROM bookmarks ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })?;
        Ok(rows
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .filter_map(|(u, t)| Url::parse(&u).ok().map(|u| (u, t)))
            .collect())
    }

    pub fn ext_kv_set(&self, ext: &str, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO ext_kv (ext, key, value) VALUES (?1, ?2, ?3)",
            params![ext, key, value],
        )?;
        Ok(())
    }

    pub fn ext_kv_get(&self, ext: &str, key: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare("SELECT value FROM ext_kv WHERE ext = ?1 AND key = ?2")?;
        let mut rows = stmt.query_map(params![ext, key], |row| row.get::<_, String>(0))?;
        Ok(rows.next().transpose()?)
    }

    pub fn ext_kv_remove(&self, ext: &str, key: &str) -> Result<()> {
        self.conn.execute("DELETE FROM ext_kv WHERE ext = ?1 AND key = ?2", params![ext, key])?;
        Ok(())
    }

    pub fn ext_kv_keys(&self, ext: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT key FROM ext_kv WHERE ext = ?1 ORDER BY key")?;
        let rows = stmt.query_map(params![ext], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_session_roundtrip() {
        let db = Db::open(":memory:").unwrap();
        let t1 = TabState {
            id: TabId(1),
            url: Some(Url::parse("https://a.example/").unwrap()),
            title: Some("A".into()),
            loading: false,
        };
        let t2 = TabState { id: TabId(2), url: None, title: None, loading: false };
        db.save_tab(0, &t1).unwrap();
        db.save_tab(1, &t2).unwrap();
        db.delete_tab(TabId(2)).unwrap();
        let restored = db.load_tabs().unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].id, TabId(1));
        assert_eq!(restored[0].url, t1.url);
        assert_eq!(restored[0].title.as_deref(), Some("A"));
    }

    #[test]
    fn history_orders_recent_first() {
        let db = Db::open(":memory:").unwrap();
        let a = Url::parse("https://a.example/").unwrap();
        let b = Url::parse("https://b.example/").unwrap();
        db.record_visit(&a, Some("A")).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        db.record_visit(&b, None).unwrap();
        let recent = db.history_recent(10).unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].0, b);
        assert_eq!(recent[1].0, a);
    }

    #[test]
    fn clear_history_removes_all_rows() {
        let db = Db::open(":memory:").unwrap();
        let url = Url::parse("https://a.example/").unwrap();
        db.record_visit(&url, Some("A")).unwrap();
        db.record_visit(&url, None).unwrap();
        assert_eq!(db.clear_history().unwrap(), 2);
        assert_eq!(db.clear_history().unwrap(), 0);
        assert!(db.history_recent(10).unwrap().is_empty());
    }

    #[test]
    fn ext_kv_namespaced_roundtrip() {
        let db = Db::open(":memory:").unwrap();
        db.ext_kv_set("e1", "k", "v1").unwrap();
        db.ext_kv_set("e2", "k", "v2").unwrap();
        assert_eq!(db.ext_kv_get("e1", "k").unwrap().as_deref(), Some("v1"));
        assert_eq!(db.ext_kv_get("e2", "k").unwrap().as_deref(), Some("v2"));
        db.ext_kv_set("e1", "k", "v1b").unwrap();
        assert_eq!(db.ext_kv_get("e1", "k").unwrap().as_deref(), Some("v1b"));
        assert_eq!(db.ext_kv_keys("e1").unwrap(), vec!["k".to_owned()]);
        db.ext_kv_remove("e1", "k").unwrap();
        assert_eq!(db.ext_kv_get("e1", "k").unwrap(), None);
    }

    #[test]
    fn bookmarks_replace_and_remove() {
        let db = Db::open(":memory:").unwrap();
        let u = Url::parse("https://x.example/").unwrap();
        db.add_bookmark(&u, Some("old")).unwrap();
        db.add_bookmark(&u, Some("new")).unwrap();
        assert_eq!(db.bookmarks().unwrap().len(), 1);
        assert_eq!(db.bookmarks().unwrap()[0].1.as_deref(), Some("new"));
        db.remove_bookmark(&u).unwrap();
        assert!(db.bookmarks().unwrap().is_empty());
    }
}

impl Db {
    /// Whole-user payload for file-based sync: bookmarks + history.
    pub fn export_all(&self) -> Result<serde_json::Value> {
        let bookmarks: Vec<serde_json::Value> = self
            .bookmarks()?
            .into_iter()
            .map(|(u, t)| serde_json::json!({ "url": u.as_str(), "title": t }))
            .collect();
        let history: Vec<serde_json::Value> = self
            .history_recent(100_000)?
            .into_iter()
            .map(|(u, t, ts)| serde_json::json!({ "url": u.as_str(), "title": t, "visitedAt": ts }))
            .collect();
        Ok(serde_json::json!({
            "format": "surf-sync",
            "version": 1,
            "exportedAt": crate::tabs::now_ms_helper(),
            "bookmarks": bookmarks,
            "history": history,
        }))
    }

    /// Merge a sync payload. Bookmarks keep the earliest created_at;
    /// history rows dedupe on (url, visited_at).
    pub fn import_all(&self, payload: &serde_json::Value) -> Result<(usize, usize)> {
        let mut bookmarks_added = 0usize;
        let mut history_added = 0usize;
        if let Some(items) = payload.get("bookmarks").and_then(|v| v.as_array()) {
            for item in items {
                let url = match item.get("url").and_then(|v| v.as_str()).and_then(|u| Url::parse(u).ok()) {
                    Some(u) => u,
                    None => continue,
                };
                let title = item.get("title").and_then(|v| v.as_str());
                let exists = self.conn
                    .query_row("SELECT 1 FROM bookmarks WHERE url = ?1", params![url.as_str()], |_| Ok(()))
                    .is_ok();
                if !exists {
                    self.add_bookmark(&url, title)?;
                    bookmarks_added += 1;
                }
            }
        }
        if let Some(items) = payload.get("history").and_then(|v| v.as_array()) {
            for item in items {
                let url = match item.get("url").and_then(|v| v.as_str()).and_then(|u| Url::parse(u).ok()) {
                    Some(u) => u,
                    None => continue,
                };
                let title = item.get("title").and_then(|v| v.as_str());
                let ts = item.get("visitedAt").and_then(|v| v.as_i64()).unwrap_or(0);
                let exists = self.conn
                    .query_row(
                        "SELECT 1 FROM history WHERE url = ?1 AND visited_at = ?2",
                        params![url.as_str(), ts],
                        |_| Ok(()),
                    )
                    .is_ok();
                if !exists {
                    self.conn.execute(
                        "INSERT INTO history (url, title, visited_at) VALUES (?1, ?2, ?3)",
                        params![url.as_str(), title, ts],
                    )?;
                    history_added += 1;
                }
            }
        }
        Ok((bookmarks_added, history_added))
    }
}

#[cfg(test)]
mod sync_tests {
    use super::*;

    #[test]
    fn export_import_roundtrip_dedupes() {
        let db = Db::open(":memory:").unwrap();
        let u = Url::parse("https://sync.example/").unwrap();
        db.add_bookmark(&u, Some("S")).unwrap();
        db.record_visit(&u, Some("S")).unwrap();
        let payload = db.export_all().unwrap();
        assert_eq!(payload["format"], "surf-sync");
        assert_eq!(payload["bookmarks"].as_array().unwrap().len(), 1);

        let other = Db::open(":memory:").unwrap();
        let (b, h) = other.import_all(&payload).unwrap();
        assert_eq!((b, h), (1, 1));
        // Re-import must not duplicate.
        let (b2, h2) = other.import_all(&payload).unwrap();
        assert_eq!((b2, h2), (0, 0));
        assert_eq!(other.bookmarks().unwrap().len(), 1);
        assert_eq!(other.history_recent(10).unwrap().len(), 1);
    }
}
