//! App icon variants and runtime switching.
//!
//! Owner decision: all three icon concepts ship, and Settings offers
//! 自動（跟隨系統主題）/ 暖灰 / 海藍 / 近黑. The bundle's static icon
//! (icons/icon.png, the light variant) stays the Finder/launch default;
//! at runtime macOS swaps the Dock icon live via
//! NSApplication.applicationIconImage. Android/iOS launcher icons are
//! build-time only and stay default (honest gap, NOTES).
//!
//! Theme detection reads `AppleInterfaceTheme` from the global
//! preferences domain; a quiet 2s poll re-applies on flip. The
//! runloop-notification machinery for a same-second response is not
//! worth the ObjC surface from Rust; 2s latency for an icon swap is
//! imperceptible next to the system's own animation.

use std::sync::atomic::{AtomicU8, Ordering};

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::state::{self, BrowserState};

pub const PREF_KEY: &str = "setting:app_icon";
pub const CHOICE_AUTO: &str = "auto";
pub const CHOICE_LIGHT: &str = "light";
pub const CHOICE_BLUE: &str = "blue";
pub const CHOICE_DARK: &str = "dark";
pub const CHOICES: [&str; 4] = [CHOICE_AUTO, CHOICE_LIGHT, CHOICE_BLUE, CHOICE_DARK];

/// The variant a preference resolves to. Auto (and any unknown value,
/// so a corrupt row can never blank the icon) follows the system.
/// Pure.
pub fn effective_variant(pref: &str, system_dark: bool) -> &'static str {
    match pref {
        CHOICE_LIGHT => CHOICE_LIGHT,
        CHOICE_BLUE => CHOICE_BLUE,
        CHOICE_DARK => CHOICE_DARK,
        _ if system_dark => CHOICE_DARK,
        _ => CHOICE_LIGHT,
    }
}

fn variant_png(variant: &str) -> &'static [u8] {
    match variant {
        CHOICE_BLUE => include_bytes!("../icons/icon-blue.png"),
        CHOICE_DARK => include_bytes!("../icons/icon-dark.png"),
        _ => include_bytes!("../icons/icon-light.png"),
    }
}

/// The stored preference; `auto` until the user picks one.
pub fn read_pref(app: &AppHandle) -> String {
    let state = app.state::<BrowserState>();
    let db = state::lock(&state.db);
    db.ext_kv_get("sys", PREF_KEY)
        .ok()
        .flatten()
        .unwrap_or_else(|| CHOICE_AUTO.to_owned())
}

/// The system's dark-mode state, read from the live appearance on the
/// main thread. The historical `AppleInterfaceTheme` preference key no
/// longer exists on this macOS (verified live: `defaults read -g
/// AppleInterfaceTheme` returns nothing while dark), so any
/// preferences-file read is a dead source; `effectiveAppearance` is
/// the canonical truth.
#[cfg(target_os = "macos")]
fn system_dark_on_main() -> bool {
    use objc2::rc::Retained;
    use objc2_app_kit::{NSApplication, NSAppearance};
    let mtm = objc2::MainThreadMarker::new().expect("must run on the main thread");
    let app: Retained<NSApplication> = NSApplication::sharedApplication(mtm);
    let appearance: Retained<NSAppearance> = app.effectiveAppearance();
    let name = appearance.name();
    name.to_string().contains("Dark")
}

#[cfg(not(target_os = "macos"))]
fn system_dark_on_main() -> bool {
    false
}

/// Swap the running app's Dock icon (macOS). Other platforms: the
/// setting is stored and applied on next macOS run; launcher icons are
/// build-time artifacts.
#[cfg(target_os = "macos")]
fn apply_variant_on_main(variant: &'static str) {
    use objc2::AnyThread;
    use objc2::rc::Retained;
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;
    let data = NSData::with_bytes(variant_png(variant));
    let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) else {
        eprintln!("[app-icon] NSImage init failed for {variant}");
        return;
    };
    let mtm = objc2::MainThreadMarker::new().expect("main thread");
    let app: Retained<NSApplication> = NSApplication::sharedApplication(mtm);
    unsafe { app.setApplicationIconImage(Some(&image)) };
}

#[cfg(not(target_os = "macos"))]
fn apply_variant_on_main(_variant: &'static str) {}

fn static_variant(idx: u8) -> &'static str {
    match CHOICES[idx as usize] {
        CHOICE_BLUE => CHOICE_BLUE,
        CHOICE_DARK => CHOICE_DARK,
        _ => CHOICE_LIGHT,
    }
}

/// Last applied variant index, so the watcher only re-applies on an
/// actual change (and re-applying twice can never fight over it).
static APPLIED: AtomicU8 = AtomicU8::new(u8::MAX);

fn variant_index(variant: &str) -> u8 {
    CHOICES.iter().position(|c| *c == variant).unwrap_or(1) as u8
}

/// Resolve the stored preference against the live theme and apply it
/// if different from what is on screen. Theme reading and icon
/// setting both belong to the main thread (effectiveAppearance and
/// setApplicationIconImage), so the whole resolution runs there.
pub fn apply_current(handle: &AppHandle) {
    let pref = read_pref(handle);
    let _ = handle.run_on_main_thread(move || {
        let dark = system_dark_on_main();
        let variant = effective_variant(&pref, dark);
        let idx = variant_index(variant);
        if APPLIED.swap(idx, Ordering::SeqCst) == idx {
            return;
        }
        eprintln!("[app-icon] applying {variant} (pref={pref}, system_dark={dark})");
        apply_variant_on_main(static_variant(idx));
    });
}

/// The 2s theme watcher: only auto follows the system, but a fixed
/// choice never changes either, so one loop covers both.
pub fn start_watcher(handle: &AppHandle) {
    let handle = handle.clone();
    std::thread::Builder::new()
        .name("app-icon-watcher".into())
        .spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(2));
            apply_current(&handle);
        })
        .expect("spawn app icon watcher");
}

/// The current icon preference for the settings panel.
#[tauri::command]
pub fn app_icon_get(app: AppHandle) -> Value {
    json!({ "choice": read_pref(&app) })
}

/// Save and apply one icon choice.
#[tauri::command]
pub fn app_icon_set(app: AppHandle, choice: String) -> Result<(), String> {
    if !CHOICES.contains(&choice.as_str()) {
        return Err(format!("未知的 icon 選項：{choice}"));
    }
    {
        let state = app.state::<BrowserState>();
        let db = state::lock(&state.db);
        let _ = db.ext_kv_set("sys", PREF_KEY, &choice);
    }
    apply_current(&app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_follows_the_system_and_fixed_choices_stay_fixed() {
        assert_eq!(effective_variant(CHOICE_AUTO, false), CHOICE_LIGHT);
        assert_eq!(effective_variant(CHOICE_AUTO, true), CHOICE_DARK);
        assert_eq!(effective_variant(CHOICE_LIGHT, true), CHOICE_LIGHT);
        assert_eq!(effective_variant(CHOICE_BLUE, false), CHOICE_BLUE);
        assert_eq!(effective_variant(CHOICE_BLUE, true), CHOICE_BLUE);
        assert_eq!(effective_variant(CHOICE_DARK, false), CHOICE_DARK);
    }

    #[test]
    fn unknown_preference_falls_back_to_auto_behavior() {
        // A corrupt row must never resolve to a missing asset.
        assert_eq!(effective_variant("", false), CHOICE_LIGHT);
        assert_eq!(effective_variant("bogus", true), CHOICE_DARK);
        let png = variant_png(effective_variant("bogus", true));
        assert!(!png.is_empty());
        assert!(!variant_png(CHOICE_BLUE).is_empty());
        assert!(!variant_png(CHOICE_LIGHT).is_empty());
    }
}
