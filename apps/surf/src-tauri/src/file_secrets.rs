//! Android secret store: the app's private data directory backs a
//! file-per-secret store (0600). Not hardware-backed — that needs an
//! Android Keystore JNI round — but persistent across restarts, unlike
//! the in-memory store. Each write rewrites the whole index atomically
//! (rename) so a crash never leaves a torn file.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use surf_core::crypto::{SecretError, SecretStore};

pub struct FileSecretStore {
    dir: PathBuf,
    entries: Mutex<BTreeMap<String, String>>,
}

impl FileSecretStore {
    pub fn new(dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&dir);
        let entries = load(&dir).unwrap_or_default();
        Self { dir, entries: Mutex::new(entries) }
    }

    fn persist(&self, map: &BTreeMap<String, String>) -> Result<(), SecretError> {
        let mut text = String::new();
        for (name, value) in map {
            // Names/values may contain newlines; escape so one secret per
            // line holds.
            text.push_str(&name.replace('\\', "\\\\").replace('\n', "\\n"));
            text.push('\t');
            text.push_str(&value.replace('\\', "\\\\").replace('\n', "\\n"));
            text.push('\n');
        }
        let tmp = self.dir.join(".secrets.tmp");
        std::fs::write(&tmp, text)
            .map_err(|e| SecretError::Store(format!("write secrets: {e}")))?;
        let target = self.dir.join("secrets.txt");
        std::fs::rename(&tmp, &target)
            .map_err(|e| SecretError::Store(format!("swap secrets: {e}")))?;
        Ok(())
    }
}

fn load(dir: &std::path::Path) -> Option<BTreeMap<String, String>> {
    let text = std::fs::read_to_string(dir.join("secrets.txt")).ok()?;
    let mut map = BTreeMap::new();
    for line in text.lines() {
        let (name, value) = line.split_once('\t')?;
        map.insert(name.replace("\\n", "\n").replace("\\\\", "\\"), value.replace("\\n", "\n").replace("\\\\", "\\"));
    }
    Some(map)
}

impl SecretStore for FileSecretStore {
    fn put(&self, name: &str, value: &str) -> Result<(), SecretError> {
        let mut map = self.entries.lock().unwrap();
        map.insert(name.to_owned(), value.to_owned());
        self.persist(&map)
    }
    fn get(&self, name: &str) -> Result<String, SecretError> {
        self.entries
            .lock()
            .unwrap()
            .get(name)
            .cloned()
            .ok_or_else(|| SecretError::NotFound(name.to_owned()))
    }
    fn delete(&self, name: &str) -> Result<(), SecretError> {
        let mut map = self.entries.lock().unwrap();
        map.remove(name);
        self.persist(&map)
    }
    fn list(&self, prefix: &str) -> Result<Vec<String>, SecretError> {
        Ok(self
            .entries
            .lock()
            .unwrap()
            .keys()
            .filter(|n| n.starts_with(prefix))
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_restart_semantics() {
        let dir = std::env::temp_dir().join(format!("surf-secrets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        {
            let store = FileSecretStore::new(dir.clone());
            store.put("pw:a.example", "secret1").unwrap();
            store.put("pw:b.example", "with\nnewline").unwrap();
            assert_eq!(store.get("pw:a.example").unwrap(), "secret1");
            assert_eq!(
                FileSecretStore::new(dir.clone()).get("pw:b.example").unwrap(),
                "with\nnewline",
                "a fresh instance reloads from disk"
            );
        }
        let reopened = FileSecretStore::new(dir.clone());
        assert_eq!(
            reopened.list("pw:").unwrap(),
            vec!["pw:a.example", "pw:b.example"]
        );
        reopened.delete("pw:a.example").unwrap();
        assert_eq!(FileSecretStore::new(dir).list("pw:").unwrap(), vec!["pw:b.example"]);
    }
}
