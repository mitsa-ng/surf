use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid manifest json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported manifestVersion {0}, expected 3")]
    Version(u32),
    #[error("missing field: {0}")]
    Missing(&'static str),
    #[error("script path escapes extension root: {0}")]
    PathEscape(String),
    #[error("referenced script missing: {0}")]
    MissingScript(String),
}

/// Surf's manifest is an MV3-inspired subset. We accept both
/// `background.serviceWorker` and `background.scripts` and normalise to
/// `scripts`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub manifest_version: u32,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub host_permissions: Vec<String>,
    #[serde(default)]
    pub background: Option<Background>,
    #[serde(default)]
    pub content_scripts: Vec<ContentScript>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Background {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scripts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_worker: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunAt {
    DocumentStart,
    DocumentEnd,
    DocumentIdle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentScript {
    pub matches: Vec<String>,
    pub js: Vec<String>,
    #[serde(default = "default_run_at")]
    pub run_at: RunAt,
}

fn default_run_at() -> RunAt {
    RunAt::DocumentIdle
}

impl Manifest {
    pub fn parse(json: &str) -> Result<Self, ManifestError> {
        let mut m: Manifest = serde_json::from_str(json)?;
        if m.manifest_version != 3 {
            return Err(ManifestError::Version(m.manifest_version));
        }
        if m.name.trim().is_empty() {
            return Err(ManifestError::Missing("name"));
        }
        if m.version.trim().is_empty() {
            return Err(ManifestError::Missing("version"));
        }
        if let Some(bg) = &mut m.background {
            if bg.scripts.is_empty() {
                if let Some(sw) = bg.service_worker.clone() {
                    bg.scripts = vec![sw];
                }
            }
        }
        Ok(m)
    }

    pub fn background_scripts(&self) -> &[String] {
        self.background.as_ref().map(|b| b.scripts.as_slice()).unwrap_or(&[])
    }
}

#[derive(Debug, Clone)]
pub struct LoadedExtension {
    pub root: PathBuf,
    pub manifest: Manifest,
}

impl LoadedExtension {
    pub fn from_dir(root: &Path) -> Result<Self, ManifestError> {
        let json = fs::read_to_string(root.join("manifest.json"))?;
        let manifest = Manifest::parse(&json)?;
        for rel in script_paths(&manifest) {
            let joined = root.join(&rel);
            let canonical = joined.canonicalize().map_err(|_| ManifestError::MissingScript(rel.clone()))?;
            let root_canonical = root.canonicalize().map_err(ManifestError::Io)?;
            if !canonical.starts_with(&root_canonical) {
                return Err(ManifestError::PathEscape(rel));
            }
            if !canonical.is_file() {
                return Err(ManifestError::MissingScript(rel));
            }
        }
        Ok(Self { root: root.to_owned(), manifest })
    }
}

fn script_paths(m: &Manifest) -> Vec<String> {
    let mut all: Vec<String> = m.background_scripts().to_vec();
    for cs in &m.content_scripts {
        all.extend(cs.js.iter().cloned());
    }
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str =
        r#"{ "manifestVersion": 3, "name": "T", "version": "1" }"#;

    #[test]
    fn parses_minimal() {
        let m = Manifest::parse(MINIMAL).unwrap();
        assert_eq!(m.name, "T");
        assert!(m.content_scripts.is_empty());
    }

    #[test]
    fn rejects_other_versions() {
        let json = r#"{ "manifestVersion": 2, "name": "T", "version": "1" }"#;
        assert!(matches!(Manifest::parse(json), Err(ManifestError::Version(2))));
    }

    #[test]
    fn service_worker_normalises_to_scripts() {
        let json = r#"{ "manifestVersion": 3, "name": "T", "version": "1",
            "background": { "serviceWorker": "bg.js" } }"#;
        let m = Manifest::parse(json).unwrap();
        assert_eq!(m.background_scripts(), &["bg.js".to_owned()]);
    }

    #[test]
    fn dir_loader_catches_path_escape() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ext");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("manifest.json"),
            r#"{ "manifestVersion": 3, "name": "T", "version": "1",
                "contentScripts": [ { "matches": ["*://*/*"], "js": ["../escape.js"] } ] }"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("escape.js"), "x").unwrap();
        assert!(matches!(
            LoadedExtension::from_dir(&root),
            Err(ManifestError::PathEscape(_))
        ));
    }
}
