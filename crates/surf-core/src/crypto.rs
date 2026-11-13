use std::sync::Mutex;
use std::collections::HashMap;

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("secret not found: {0}")]
    NotFound(String),
    #[error("store failure: {0}")]
    Store(String),
}

pub trait SecretStore {
    fn put(&self, name: &str, value: &str) -> Result<(), SecretError>;
    fn get(&self, name: &str) -> Result<String, SecretError>;
    fn delete(&self, name: &str) -> Result<(), SecretError>;
    /// Names of stored secrets starting with `prefix`, sorted. Used by the
    /// saved-passwords list; the keychain implements it via SecItem search.
    fn list(&self, prefix: &str) -> Result<Vec<String>, SecretError>;
}

/// In-memory store; swapped for the OS keychain in the Tauri shell on
/// platforms that have one. Send + Sync via internal Mutex.
pub struct MemorySecretStore {
    entries: Mutex<HashMap<String, String>>,
}

impl MemorySecretStore {
    pub fn new() -> Self {
        Self { entries: Mutex::new(HashMap::new()) }
    }
}

impl Default for MemorySecretStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretStore for MemorySecretStore {
    fn put(&self, name: &str, value: &str) -> Result<(), SecretError> {
        self.entries.lock().unwrap().insert(name.to_owned(), value.to_owned());
        Ok(())
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
        self.entries.lock().unwrap().remove(name);
        Ok(())
    }
    fn list(&self, prefix: &str) -> Result<Vec<String>, SecretError> {
        let mut names: Vec<String> = self
            .entries
            .lock()
            .unwrap()
            .keys()
            .filter(|n| n.starts_with(prefix))
            .cloned()
            .collect();
        names.sort();
        Ok(names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let s = MemorySecretStore::new();
        s.put("sync-key", "v1").unwrap();
        assert_eq!(s.get("sync-key").unwrap(), "v1");
        s.delete("sync-key").unwrap();
        assert!(matches!(s.get("sync-key"), Err(SecretError::NotFound(_))));
    }

    #[test]
    fn list_filters_by_prefix() {
        let s = MemorySecretStore::new();
        s.put("pw:a.example", "x").unwrap();
        s.put("pw:b.example", "y").unwrap();
        s.put("other", "z").unwrap();
        assert_eq!(s.list("pw:").unwrap(), vec!["pw:a.example", "pw:b.example"]);
        assert!(s.list("none:").unwrap().is_empty());
    }
}
