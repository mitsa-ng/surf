//! ext://<ext-id>/<path> serves unpacked extension files to popups,
//! options pages, and content-script frames. Path traversal is rejected
//! and every response carries the resolved file's real location.

use std::borrow::Cow;
use std::path::PathBuf;

use tauri::Manager;

fn mime_for(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn resolve(app: &tauri::AppHandle, ext_id: &str, rel: &str) -> Option<PathBuf> {
    let state = app.state::<crate::state::BrowserState>();
    let registry = crate::state::lock(&state.registry);
    let ext = registry.get(&surf_ext_host::ExtensionId(ext_id.to_owned()))?;
    let root = ext.root.canonicalize().ok()?;
    let target = root.join(rel).canonicalize().ok()?;
    if !target.starts_with(&root) || !target.is_file() {
        return None;
    }
    Some(target)
}

pub fn handle(
    ctx: tauri::UriSchemeContext<'_, tauri::Wry>,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Cow<'static, [u8]>> {

    let uri = request.uri().to_string();
    // strip "ext://<id>/<path>"; host carries the id, path the file.
    let rest = &uri["ext://".len()..];
    let (ext_id, rel) = match rest.split_once('/') {
        Some((id, p)) => (id, p),
        None => (rest, ""),
    };
    let rel = rel.split('?').next().unwrap_or("");
    let rel = if rel.is_empty() { "popup.html" } else { rel };

    match resolve(ctx.app_handle(), ext_id, rel) {
        Some(path) => match std::fs::read(&path) {
            Ok(bytes) => tauri::http::Response::builder()
                .status(200)
                .header("Content-Type", mime_for(rel))
                .body(bytes.into())
                .unwrap_or_else(|_| empty(404)),
            Err(_) => empty(404),
        },
        None => empty(404),
    }
}

fn empty(status: u16) -> tauri::http::Response<Cow<'static, [u8]>> {
    tauri::http::Response::builder()
        .status(status)
        .body(Cow::Borrowed(&[][..]))
        .expect("static response")
}
