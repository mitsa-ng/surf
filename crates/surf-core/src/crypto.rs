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
}
