//! Top-level navigation filtering. The adblock engine is !Send (Rc
//! internals), so it lives on its own thread and the synchronous
//! on_navigation hook does a channel roundtrip to ask for a verdict.

use std::sync::mpsc::{Receiver, Sender, SyncSender};
use std::sync::Mutex;

use surf_ext_host::rules::{AdblockEngine, RequestAction, RulesEngine};
use url::Url;

pub struct NavFilter {
    tx: Mutex<Option<Sender<Query>>>,
}

struct Query {
    url: Url,
    reply: SyncSender<bool>,
}

impl Default for NavFilter {
    fn default() -> Self {
        Self { tx: Mutex::new(None) }
    }
}

impl NavFilter {
    /// Start (or restart) the engine thread with the given filters. The
    /// old thread shuts down when the sender it holds is dropped.
    pub fn start(&self, filters: Vec<String>) {
        let (tx, rx) = std::sync::mpsc::channel::<Query>();
        std::thread::Builder::new()
            .name("nav-filter".into())
            .spawn(move || run_engine(rx, filters))
            .expect("spawn nav filter");
        *self.tx.lock().unwrap() = Some(tx);
    }

    /// Blocking verdict for a top-level navigation. Fail-open: engine
    /// unavailable means allow.
    pub fn allows(&self, url: &Url) -> bool {
        let tx = self.tx.lock().unwrap();
        let Some(tx) = tx.as_ref() else {
            return true;
        };
        let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel(1);
        if tx.send(Query { url: url.clone(), reply: reply_tx }).is_err() {
            return true;
        }
        reply_rx.recv().unwrap_or(true)
    }
}

fn run_engine(rx: Receiver<Query>, filters: Vec<String>) {
    let engine = match AdblockEngine::from_filters(filters.iter().map(String::as_str)) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("nav filter load failed: {e}");
            return;
        }
    };
    while let Ok(q) = rx.recv() {
        let source = None;
        let blocked = engine.decide(&q.url, source) == RequestAction::Block;
        let _ = q.reply.send(!blocked);
    }
}

/// Load the persisted filter list (one EasyList-syntax rule per line).
pub fn load_filters(db: &surf_core::storage::Db) -> Vec<String> {
    db.ext_kv_get("sys", "nav_filters")
        .ok()
        .flatten()
        .map(|text| text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('!')).map(str::to_owned).collect())
        .unwrap_or_default()
}

pub fn save_filters(db: &surf_core::storage::Db, filters: &[String]) -> Result<(), surf_core::storage::StorageError> {
    db.ext_kv_set("sys", "nav_filters", &filters.join("\n"))
}
