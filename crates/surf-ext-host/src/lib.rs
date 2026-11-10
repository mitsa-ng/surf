pub mod api;
pub mod manifest;
pub mod permission;
pub mod planner;
pub mod router;
pub mod rules;
pub mod runtime;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use manifest::{LoadedExtension, ManifestError};

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ExtensionId(pub String);

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("extension not found: {0}")]
    NotFound(String),
    #[error("duplicate extension id: {0}")]
    Duplicate(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("manifest: {0}")]
    Manifest(#[from] ManifestError),
}

/// In-memory registry of installed extensions. Persistence moves to
/// surf-core storage in M2.
#[derive(Default)]
pub struct Registry {
    extensions: HashMap<ExtensionId, InstalledExtension>,
}

#[derive(Debug, Clone)]
pub struct InstalledExtension {
    pub id: ExtensionId,
    pub root: PathBuf,
    pub manifest: manifest::Manifest,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Install an unpacked extension directory. The id is a slug of the
    /// manifest name; reinstalling the same id replaces the previous version.
    pub fn install_dir(&mut self, path: impl AsRef<Path>) -> Result<ExtensionId, RegistryError> {
        let loaded = LoadedExtension::from_dir(path.as_ref())?;
        let id = ExtensionId(slug(&loaded.manifest.name));
        self.extensions.insert(
            id.clone(),
            InstalledExtension { id: id.clone(), root: loaded.root, manifest: loaded.manifest },
        );
        Ok(id)
    }

    pub fn uninstall(&mut self, id: &ExtensionId) -> Result<(), RegistryError> {
        self.extensions.remove(id).map(|_| ()).ok_or_else(|| RegistryError::NotFound(id.0.clone()))
    }

    pub fn get(&self, id: &ExtensionId) -> Option<&InstalledExtension> {
        self.extensions.get(id)
    }

    pub fn list(&self) -> Vec<&InstalledExtension> {
        self.extensions.values().collect()
    }
}

fn slug(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let trimmed = s.trim_matches('-').to_owned();
    if trimmed.is_empty() { "extension".to_owned() } else { trimmed }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn install_and_replace() {
        let dir = tempfile::tempdir().unwrap();
        let ext = dir.path().join("hello");
        fs::create_dir_all(&ext).unwrap();
        fs::write(
            ext.join("manifest.json"),
            r#"{ "manifestVersion": 3, "name": "Hello World", "version": "1.0",
                 "contentScripts": [ { "matches": ["https://example.com/*"], "js": ["cs.js"] } ] }"#,
        )
        .unwrap();
        fs::write(ext.join("cs.js"), "console.log('hi')").unwrap();

        let mut reg = Registry::new();
        let id = reg.install_dir(&ext).unwrap();
        assert_eq!(id, ExtensionId("hello-world".into()));
        assert_eq!(reg.list().len(), 1);
        let same = reg.install_dir(&ext).unwrap();
        assert_eq!(same, id);
        assert_eq!(reg.list().len(), 1);
        reg.uninstall(&id).unwrap();
        assert!(reg.get(&id).is_none());
    }
}
