//! WKContentRuleList glue (Apple platforms). Compiles a WebKit
//! content-blocker JSON list and installs it on a live webview's user
//! content controller, giving subresource blocking on top of the Rust
//! navigation filter. `with_webview` runs on the main thread, which
//! `WKContentRuleListStore` requires.

use std::collections::HashSet;
use std::sync::Mutex;

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_foundation::NSString;
use objc2_web_kit::{WKContentRuleList, WKContentRuleListStore, WKUserContentController};
use tauri::{AppHandle, Manager, Wry};

/// Labels that already carry a rule list from a previous install; only
/// those need a remove before re-adding. Removing on a controller that
/// never had one trips a WebKit proxy race, so first install skips it.
static INSTALLED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// Compile `json` (WebKit content-blocker format) and install it on the
/// webview registered under `label`. Rule edits converge because each
/// install replaces the previous list.
///
/// Disabled unless `SURF_WK_CONTENT_RULES=1`: on this WebKit build
/// (macOS 27.2) `-[WKUserContentController addContentRuleList:]`
/// segfaults on a null proxy even from plain Swift with a fully loaded
/// page (repro: docs/wkcontentrulelist-crash-repro.swift). The flag lets
/// the integration be re-enabled the moment WebKit fixes it, without
/// shipping a crasher.
pub fn install(app: &AppHandle<Wry>, label: &str, json: &str) {
    if std::env::var("SURF_WK_CONTENT_RULES").ok().as_deref() != Some("1") {
        static NOTED: std::sync::Once = std::sync::Once::new();
        NOTED.call_once(|| {
            eprintln!("[wk-rules] content-rule install disabled (WebKit addContentRuleList regression); set SURF_WK_CONTENT_RULES=1 to enable");
        });
        return;
    }
    let Some(wv) = app.get_webview(label) else {
        return;
    };
    let json = json.to_owned();
    let label = label.to_owned();
    let _ = wv.with_webview(move |platform| {
        let mtm = MainThreadMarker::new().expect("with_webview runs on the main thread");
        // tauri's controller() is the WKUserContentController wry itself
        // created the webview with.
        let ucc: Retained<WKUserContentController> = unsafe {
            Retained::retain_autoreleased(platform.controller() as *mut WKUserContentController)
                .expect("controller")
        };
        let had_rules = INSTALLED
            .lock()
            .unwrap()
            .get_or_insert_with(HashSet::new)
            .remove(&label);
        if had_rules {
            unsafe { ucc.removeAllContentRuleLists() };
        }
        if json.is_empty() {
            return;
        }
        let Some(store) = (unsafe { WKContentRuleListStore::defaultStore(mtm) }) else {
            eprintln!("[wk-rules] no default content rule list store");
            return;
        };
        let json_str = NSString::from_str(&json);
        let label_for_block = label.clone();
        let handler = block2::RcBlock::new(
            move |rules: *mut WKContentRuleList, err: *mut objc2_foundation::NSError| {
                if !err.is_null() {
                    let msg = unsafe { (*err).localizedDescription().to_string() };
                    eprintln!("[wk-rules] compile failed: {msg}");
                    return;
                }
                if rules.is_null() {
                    eprintln!("[wk-rules] compile returned no rules");
                    return;
                }
                eprintln!("[wk-rules] compiled ok, adding to {label_for_block}");
                unsafe { ucc.addContentRuleList(&*rules) };
                INSTALLED
                    .lock()
                    .unwrap()
                    .get_or_insert_with(HashSet::new)
                    .insert(label_for_block.clone());
                eprintln!("[wk-rules] installed on {label_for_block}");
            },
        );
        unsafe {
            store.compileContentRuleListForIdentifier_encodedContentRuleList_completionHandler(
                None,
                Some(&json_str),
                Some(&handler),
            );
        }
        eprintln!("[wk-rules] content blocker compile requested for {label}");
    });
}
