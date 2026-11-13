//! Favicon pipeline: fetch `<origin>/favicon.ico` once per origin, cache
//! the bytes on disk, and serve them to every webview through the
//! `favicon://` custom scheme (works for the chrome UI and the NTP alike,
//! no asset-protocol scope needed). Sites without a usable icon keep the
//! first-letter placeholder; nothing is invented.

use std::borrow::Cow;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager};
use url::Url;

/// Response cap: favicons are tiny; anything bigger is not a favicon.
const MAX_BYTES: usize = 512 * 1024;

pub struct FaviconCache {
    dir: PathBuf,
    /// origin -> cached asset url; None = this origin has no usable icon
    /// (remembered so we do not re-fetch on every render).
    seen: Mutex<HashMap<String, Option<String>>>,
}

impl FaviconCache {
    pub fn new(dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&dir);
        Self { dir, seen: Mutex::new(HashMap::new()) }
    }
}

/// `https://host[:port]` for http(s) urls; None otherwise.
fn origin_of(url: &Url) -> Option<String> {
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let host = url.host_str()?;
    let port = url.port().map(|p| format!(":{p}")).unwrap_or_default();
    Some(format!("{}://{host}{port}", url.scheme()))
}

fn file_name_for(origin: &str, ext: &str) -> String {
    let mut h = DefaultHasher::new();
    origin.hash(&mut h);
    format!("{:016x}.{ext}", h.finish())
}

/// Sniff the image type from leading bytes; returns the extension.
fn sniff(buf: &[u8]) -> Option<&'static str> {
    if buf.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("png")
    } else if buf.starts_with(b"GIF8") {
        Some("gif")
    } else if buf.starts_with(&[0xFF, 0xD8]) {
        Some("jpg")
    } else if buf.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        Some("ico")
    } else if buf.starts_with(b"<svg") || buf.starts_with(b"<?xml") {
        Some("svg")
    } else if buf.starts_with(b"BM") {
        Some("bmp")
    } else {
        None
    }
}

fn content_type(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "gif" => "image/gif",
        "jpg" => "image/jpeg",
        "ico" => "image/x-icon",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        _ => "application/octet-stream",
    }
}

/// Asset url the webviews load the cached icon from.
fn asset_url(file: &str) -> String {
    format!("favicon://local/{file}")
}

/// Fetch (or recall) the favicon for a page url. Blocking; call from a
/// command thread. Returns the `favicon://` url or None.
pub fn lookup(app: &AppHandle, url: &Url) -> Option<String> {
    let state = app.state::<crate::state::BrowserState>();
    let origin = origin_of(url)?;
    if let Some(hit) = state.favicons.seen.lock().unwrap().get(&origin) {
        return hit.clone();
    }
    let result = fetch(&state.favicons.dir, &origin);
    state.favicons.seen.lock().unwrap().insert(origin, result.clone());
    result
}

fn fetch(dir: &Path, origin: &str) -> Option<String> {
    let target = format!("{origin}/favicon.ico");
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(4))
        .timeout(Duration::from_secs(6))
        .build();
    let resp = agent.get(&target).call().ok()?;
    if resp.status() != 200 {
        return None;
    }
    let mut bytes = Vec::new();
    use std::io::Read;
    resp.into_reader().take(MAX_BYTES as u64).read_to_end(&mut bytes).ok()?;
    let ext = sniff(&bytes)?;
    let name = file_name_for(origin, ext);
    let path = dir.join(&name);
    std::fs::write(&path, &bytes).ok()?;
    Some(asset_url(&name))
}

/// `favicon://` scheme handler: serves only cached files by basename, so
/// the scheme can never read outside the favicon directory.
pub fn handle_asset(
    ctx: tauri::UriSchemeContext<'_, tauri::Wry>,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Cow<'static, [u8]>> {
    let app = ctx.app_handle();
    let state = app.state::<crate::state::BrowserState>();
    let uri = request.uri().to_string();
    // favicon://local/<name> (the host segment is cosmetic).
    let name = uri.rsplit('/').next().unwrap_or("");
    let clean: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '.').collect();
    let path = if clean.is_empty() || clean.starts_with('.') || clean.contains("..") {
        None
    } else {
        Some(state.favicons.dir.join(&clean))
    };
    match path.filter(|p| p.is_file()).and_then(|p| std::fs::read(&p).ok().map(|bytes| (p, bytes))) {
        Some((_, bytes)) => {
            let ext = clean.rsplit('.').next().unwrap_or("");
            tauri::http::Response::builder()
                .status(200)
                .header("Content-Type", content_type(ext))
                .header("Cache-Control", "max-age=604800")
                .body(bytes.into())
                .unwrap_or_else(|_| empty(404))
        }
        _ => empty(404),
    }
}

fn empty(status: u16) -> tauri::http::Response<Cow<'static, [u8]>> {
    tauri::http::Response::builder()
        .status(status)
        .body(Cow::Borrowed(&[][..]))
        .expect("static response")
}

/// Command surface: the chrome UI asks per rendered url.
#[tauri::command]
pub fn favicon_url(app: AppHandle, url: String) -> Option<String> {
    let parsed = Url::parse(&url).ok()?;
    lookup(&app, &parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_and_names_are_stable() {
        let a = Url::parse("https://example.org/x?y=1").unwrap();
        let b = Url::parse("https://example.org/z").unwrap();
        assert_eq!(origin_of(&a), Some("https://example.org".into()));
        assert_eq!(origin_of(&a), origin_of(&b));
        // Ports survive; non-http schemes have no origin.
        assert_eq!(origin_of(&Url::parse("http://a.example:8080/").unwrap()), Some("http://a.example:8080".into()));
        assert_eq!(origin_of(&Url::parse("file:///tmp/x").unwrap()), None);
        let name = file_name_for("https://example.org", "png");
        assert!(name.ends_with(".png"), "{name}");
        assert_eq!(name, file_name_for("https://example.org", "png"));
    }

    #[test]
    fn sniff_matches_magic_bytes() {
        assert_eq!(sniff(&[0x89, b'P', b'N', b'G', 0, 0]), Some("png"));
        assert_eq!(sniff(&[0x00, 0x00, 0x01, 0x00]), Some("ico"));
        assert_eq!(sniff(b"<svg xmlns='x'>"), Some("svg"));
        assert_eq!(sniff(b"not an image"), None);
    }
}
