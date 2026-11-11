//! File-based auto sync (desktop). The user points Surf at one
//! `surf-sync.json` inside any cloud-synced folder (iCloud Drive,
//! Dropbox, Syncthing, ...); a worker reconciles the database and the
//! file every few seconds. Local changes are exported; external changes
//! are merged with the same dedupe-merge used by manual import, then the
//! merged state is written back. No server, no account.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::state::{self, BrowserState};

const TICK: Duration = Duration::from_secs(5);

struct SyncConfig {
    path: PathBuf,
    last_db_hash: Option<u64>,
    last_file_hash: Option<u64>,
}

#[derive(Default)]
pub struct AutoSync {
    slot: Mutex<Option<SyncConfig>>,
}

impl AutoSync {
    /// Point the sync at `path`, or stop it with `None`. The next tick
    /// bootstraps: an existing file with different content is merged
    /// first, otherwise the file is created from the local database.
    pub fn configure(&self, path: Option<PathBuf>) {
        *self.slot.lock().unwrap() = path.map(|path| SyncConfig {
            path,
            last_db_hash: None,
            last_file_hash: None,
        });
    }

    fn tick(&self, app: &AppHandle) {
        let mut guard = self.slot.lock().unwrap();
        let Some(cfg) = guard.as_mut() else { return };

        let state = app.state::<BrowserState>();
        let payload = {
            let db = state::lock(&state.db);
            match db.export_all() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("[auto-sync] export failed: {e}");
                    return;
                }
            }
        };
        let mut db_text = serde_json::to_string(&payload).unwrap_or_default();
        let h_db = hash_str(&db_text);

        // External change (or first run against an existing file): merge
        // it in, then fall through so the merged state is written back.
        let file_text = std::fs::read_to_string(&cfg.path).ok();
        let h_file = file_text.as_deref().map(hash_str);
        if let (Some(text), Some(hf)) = (&file_text, h_file) {
            if cfg.last_file_hash != Some(hf) && hf != h_db {
                let payload: serde_json::Value = match serde_json::from_str(text) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("[auto-sync] file is not valid JSON, skipping: {e}");
                        // Remember it so a corrupt write is not re-read every tick.
                        cfg.last_file_hash = Some(hf);
                        return;
                    }
                };
                let db = state::lock(&state.db);
                match db.import_all(&payload) {
                    Ok((b, h)) if b > 0 || h > 0 => eprintln!("[auto-sync] merged +{b} bookmarks, +{h} history"),
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("[auto-sync] import failed: {e}");
                        cfg.last_file_hash = Some(hf);
                        return;
                    }
                }
                drop(db);
                // Re-export so the file reflects the merged state below.
                let db = state::lock(&state.db);
                if let Ok(merged) = db.export_all() {
                    db_text = serde_json::to_string(&merged).unwrap_or_default();
                }
            }
        }
        let h_db = hash_str(&db_text);

        if cfg.last_db_hash != Some(h_db) || h_file != Some(h_db) {
            if let Err(e) = write_atomic(&cfg.path, &db_text) {
                eprintln!("[auto-sync] write failed: {e}");
                return;
            }
            cfg.last_file_hash = Some(h_db);
        }
        cfg.last_db_hash = Some(h_db);
    }
}

fn hash_str(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

fn write_atomic(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("surf-tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// Start the reconciler thread. Called once from setup; the loop runs
/// until the app exits.
pub fn start(app: AppHandle) {
    let auto_sync = app.state::<BrowserState>().auto_sync.clone();
    std::thread::Builder::new()
        .name("auto-sync".into())
        .spawn(move || loop {
            std::thread::sleep(TICK);
            auto_sync.tick(&app);
        })
        .expect("spawn auto sync");
}
