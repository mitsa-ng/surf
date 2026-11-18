use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use surf_core::storage::Db;
use surf_core::tabs::{TabId, TabModel, TabState};
use surf_ext_host::permission::PermissionManager;
use surf_ext_host::planner::InjectionPlanner;
use surf_ext_host::router::Router;
use surf_ext_host::Registry;
use tauri::{AppHandle, Manager, Window, Wry};
#[cfg(desktop)]
use tauri::WindowBuilder;
use url::Url;

use crate::api_impl::AlarmState;
use crate::bg::{BgMsg, BgRegistry};
use crate::content_bridge::TokenRegistry;
use crate::layout;
use crate::tabview::TauriTabView;

/// A closed tab remembered for reopen (Cmd+Shift+T).
#[derive(Clone)]
pub struct ClosedTabInfo {
    pub url: Option<url::Url>,
    pub title: Option<String>,
}

/// A submitted login awaiting the save prompt. The password lives here
/// only until the user answers the prompt; it never reaches the chrome
/// webview or the database, only the OS keychain.
#[derive(Clone)]
pub struct PendingPassword {
    pub host: String,
    pub user: String,
    pub password: String,
}

#[derive(serde::Serialize, Clone)]
pub struct TabsSnapshot {
    pub tabs: Vec<TabState>,
    pub active: Option<TabId>,
    /// Split view members, if active.
    pub split: Option<(TabId, TabId)>,
    /// The top-level navigation the filter just rejected, so the chrome
    /// can name what stopped it instead of reading as a dead browser.
    /// Rides the tabs snapshot (a channel proven to deliver live) rather
    /// than a bespoke event that raced on macOS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked: Option<BlockedNav>,
}

#[derive(serde::Serialize, Clone)]
pub struct BlockedNav {
    pub tab: u32,
    pub url: String,
    /// Bumps on every fresh block so the UI can tell a new rejection from
    /// the follow-up snapshots that still carry this one.
    pub seq: u64,
}

pub struct BrowserState {
    pub panel_open: AtomicBool,
    /// Any chrome overlay (tab menu, tab search, address suggestions)
    /// needs room below the chrome stack; when set, relayout expands the
    /// chrome webview to the full window like the panel does.
    pub overlay_open: AtomicBool,
    /// Desktop bookmarks bar visibility; mirrors the persisted
    /// `setting:bookmarks_bar` and feeds the chrome height.
    pub bookmarks_bar: AtomicBool,
    pub model: Mutex<TabModel>,
    pub db: Arc<Mutex<Db>>,
    pub registry: Mutex<Registry>,
    pub webviews: Mutex<HashMap<TabId, TauriTabView>>,
    pub cs_tokens: TokenRegistry,
    pub router: Mutex<Router>,
    pub planner: Mutex<InjectionPlanner>,
    pub permission: Arc<Mutex<PermissionManager>>,
    pub alarms: Arc<Mutex<HashMap<(String, String), AlarmState>>>,
    pub bg_threads: Mutex<BgRegistry>,
    /// Platform secret store for sync/crypto (keychain on macOS). Read
    /// paths activate with the sync milestone.
    #[allow(dead_code)]
    pub secrets: std::sync::Arc<dyn surf_core::crypto::SecretStore + Send + Sync>,
    /// Top-level navigation filter engine (dedicated thread).
    pub nav_filter: crate::nav_filter::NavFilter,
    /// Download history for the downloads panel (newest first).
    pub downloads: Mutex<Vec<crate::commands::DownloadRecord>>,
    /// Favicon disk cache + per-origin results (see favicon.rs).
    pub favicons: crate::favicon::FaviconCache,
    /// A login form submission awaiting the save prompt (see commands).
    pub pending_pw: Mutex<Option<PendingPassword>>,
    /// Recently closed tabs (for Cmd+Shift+T), newest last. Private tabs
    /// are never recorded.
    pub recently_closed: Mutex<Vec<crate::state::ClosedTabInfo>>,
    /// The most recent filter-rejected navigation; cleared when a real
    /// navigation commits (see record_navigation).
    pub blocked_nav: Mutex<Option<BlockedNav>>,
    /// Split view: two tab ids shown side by side (desktop only, not
    /// persisted; a view state, not session state).
    pub split: Mutex<Option<(TabId, TabId)>>,
    /// Mobile: the rendered bottom bar's height in physical px, reported
    /// by the UI (element height x devicePixelRatio). Tauri reports
    /// logical==physical on Android (scale 1.0) while the webviews
    /// render CSS at their own density, so the reserve must come
    /// measured, not computed from a logical constant.
    #[cfg(mobile)]
    pub bottom_bar_px: std::sync::atomic::AtomicU32,
    /// Mobile: the chrome webview's own physical viewport, measured by
    /// the UI on resize. tao keeps reporting the pre-inset window size
    /// (system bars and IME shrink the decor frame without a tao resize
    /// event), so the layout must trust the webview's live numbers.
    #[cfg(mobile)]
    pub chrome_viewport_w_px: std::sync::atomic::AtomicU32,
    #[cfg(mobile)]
    pub chrome_viewport_h_px: std::sync::atomic::AtomicU32,
    /// A deep link that arrived at cold start, before the chrome UI
    /// mounted; the UI takes it once the tab model is ready.
    pub launch_deeplink: Mutex<Option<String>>,
    /// File-based auto sync reconciler state (desktop).
    #[cfg(desktop)]
    pub auto_sync: std::sync::Arc<crate::auto_sync::AutoSync>,
}

impl BrowserState {
    /// `secrets_dir` backs the Android file secret store; ignored where
    /// the platform keychain (macOS) or in-memory fallback applies.
    pub fn new(db: Db, favicon_dir: std::path::PathBuf, secrets_dir: Option<std::path::PathBuf>) -> Self {
        let _ = &secrets_dir;
        Self {
            panel_open: AtomicBool::new(false),
            overlay_open: AtomicBool::new(false),
            bookmarks_bar: AtomicBool::new(false),
            model: Mutex::new(TabModel::new()),
            db: Arc::new(Mutex::new(db)),
            registry: Mutex::new(Registry::new()),
            webviews: Mutex::new(HashMap::new()),
            cs_tokens: TokenRegistry::default(),
            router: Mutex::new(Router::new()),
            planner: Mutex::new(InjectionPlanner),
            permission: Arc::new(Mutex::new(PermissionManager::new())),
            alarms: Arc::new(Mutex::new(HashMap::new())),
            bg_threads: Mutex::new(BgRegistry::default()),
            #[cfg(target_os = "macos")]
            secrets: std::sync::Arc::new(crate::keychain::KeychainStore::new("com.surf.browser")),
            #[cfg(target_os = "android")]
            secrets: std::sync::Arc::new(crate::file_secrets::FileSecretStore::new(
                secrets_dir.unwrap_or_else(|| std::path::PathBuf::from(".")),
            )),
            #[cfg(not(any(target_os = "macos", target_os = "android")))]
            secrets: {
                let _ = secrets_dir;
                std::sync::Arc::new(surf_core::crypto::MemorySecretStore::new())
            },
            nav_filter: crate::nav_filter::NavFilter::default(),
            downloads: Mutex::new(Vec::new()),
            favicons: crate::favicon::FaviconCache::new(favicon_dir),
            pending_pw: Mutex::new(None),
            recently_closed: Mutex::new(Vec::new()),
            blocked_nav: Mutex::new(None),
            split: Mutex::new(None),
            #[cfg(mobile)]
            // 220 physical px ≈ 81 CSS px at 2.625 density, the measured
            // emulator bar; the UI corrects it right after mount.
            bottom_bar_px: std::sync::atomic::AtomicU32::new(220),
            #[cfg(mobile)]
            chrome_viewport_w_px: std::sync::atomic::AtomicU32::new(0),
            #[cfg(mobile)]
            chrome_viewport_h_px: std::sync::atomic::AtomicU32::new(0),
            launch_deeplink: Mutex::new(None),
            #[cfg(desktop)]
            auto_sync: std::sync::Arc::new(crate::auto_sync::AutoSync::default()),
        }
    }
}

pub fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Main window label; also the chrome webview label on desktop.
pub const MAIN_LABEL: &str = "main";

pub fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();

    #[cfg(desktop)]
    {
        let builder = WindowBuilder::new(app, MAIN_LABEL);
        #[cfg(target_os = "macos")]
        // Chrome-style: the chrome stack covers the window top and the
        // traffic lights float over the tab strip, so the content view
        // must extend under the (hidden) title bar.
        let builder = builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true);
        let window = builder
            .title("Surf")
            .inner_size(1200.0, 800.0)
            .min_inner_size(480.0, 480.0)
            .build()?;

        {
            let scale = window.scale_factor().unwrap_or(1.0);
            let size = window.inner_size()?;
            let logical_size = size.to_logical::<f64>(scale);
            let chrome_h = layout::CHROME_HEIGHT_LOGICAL;
            let chrome = window.add_child(
                tauri::webview::WebviewBuilder::new("chrome", tauri::WebviewUrl::App("index.html".into()))
                    .transparent(true)
                    .focused(true),
                tauri::LogicalPosition::new(0.0, 0.0),
                tauri::LogicalSize::new(logical_size.width, chrome_h),
            )?;
            drop(chrome);
        }

        restore_session(&handle);
        watch_resize(&handle, &window);
        if let Err(e) = crate::menus::build(&handle) {
            eprintln!("menu build failed: {e}");
        }
        crate::menus::wire_events(&handle);
    }

    #[cfg(mobile)]
    {
        let win = tauri::WebviewWindowBuilder::new(app, MAIN_LABEL, tauri::WebviewUrl::App("index.html".into()))
            .title("Surf")
            .build()?;
        restore_session(&handle);
        // Rotation changes the window size; the content webviews must
        // re-stack above the bottom bar.
        if let Some(window) = app.get_window(MAIN_LABEL) {
            watch_resize(&handle, &window);
        }
        drop(win);
    }

    crate::deep_links::wire(&handle);
    // The running app's icon follows the stored preference (auto =
    // system theme); the watcher covers theme flips while running.
    crate::app_icon::apply_current(&handle);
    crate::app_icon::start_watcher(&handle);
    {
        let state = app.state::<BrowserState>();
        let filters = crate::nav_filter::load_filters(&lock(&state.db));
        if !filters.is_empty() {
            state.nav_filter.start(filters);
        }
        // Bookmarks bar visibility persists across launches.
        let bar = lock(&state.db)
            .ext_kv_get("sys", "setting:bookmarks_bar")
            .ok()
            .flatten()
            .is_some_and(|v| !v.is_empty());
        state.bookmarks_bar.store(bar, std::sync::atomic::Ordering::Relaxed);
        // Downloads persist across launches (newest first, capped at 100).
        {
            let rows = lock(&state.db).downloads_all(100).unwrap_or_default();
            *lock(&state.downloads) = rows
                .into_iter()
                .map(|r| crate::commands::DownloadRecord {
                    url: r.url,
                    path: r.path,
                    filename: r.filename,
                    success: r.success,
                    at: r.at,
                })
                .collect();
        }
    }
    #[cfg(desktop)]
    {
        let path = {
            let state = app.state::<BrowserState>();
            let db = lock(&state.db);
            db.ext_kv_get("sys", "setting:sync_file").ok().flatten()
        };
        app.state::<BrowserState>().auto_sync.configure(path.map(std::path::PathBuf::from));
        crate::auto_sync::start(handle.clone());
    }
    restore_extensions(&handle);
    dev_auto_install(&handle);
    start_alarm_scheduler(&handle);
    #[cfg(debug_assertions)]
    dev_host_log_sink(&handle);
    Ok(())
}

/// Dev observability: log events extensions route to the host (console.*,
/// errors) so extension failures are visible in the terminal.
#[cfg(debug_assertions)]
fn dev_host_log_sink(app: &AppHandle) {
    let state = app.state::<BrowserState>();
    struct LogSink;
    impl surf_ext_host::router::MessageSink for LogSink {
        fn deliver(&self, envelope: surf_ext_host::router::Envelope) -> Result<(), surf_ext_host::router::RouterError> {
            if let surf_ext_host::router::MessageBody::Event { name, payload } = &envelope.body {
                eprintln!("[ext-host] {name}: {payload}");
            }
            Ok(())
        }
    }
    lock(&state.router).subscribe(
        surf_ext_host::router::Endpoint::Host,
        std::sync::Arc::new(LogSink),
    );
}

/// SURF_DEV_EXT_PATH installs a fixture extension with full grants at
/// startup. Dev affordance only; the release build ships without it.
fn dev_auto_install(app: &AppHandle) {
    let Ok(path) = std::env::var("SURF_DEV_EXT_PATH") else {
        return;
    };
    let state = app.state::<BrowserState>();
    let id = {
        let mut registry = lock(&state.registry);
        match registry.install_dir(&path) {
            Ok(id) => {
                // Grant before the thread starts: bg scripts run
                // immediately and must see their permissions.
                {
                    let mut pm = lock(&state.permission);
                    for api in ["storage", "tabs", "bookmarks", "alarms", "fetch"] {
                        pm.store_mut().grant_api(&id, api);
                    }
                }
                let ext = registry.get(&id).cloned();
                drop(registry);
                if let Some(ext) = ext {
                    lock(&state.bg_threads).start(app, ext);
                }
                id
            }
            Err(e) => {
                eprintln!("dev extension install failed: {e}");
                return;
            }
        }
    };
    persist_extension_state(app);
    eprintln!("dev extension installed: {}", id.0);
    // Open the fixture popup so ext:// scheme, capability, and ext_api_call
    // are exercised without manual UI steps.
    if std::env::var("SURF_DEV_EXT_PATH").is_ok() {
        let _ = crate::commands::ext_popup_open(app.clone(), id.0.clone());
    }
}

/// Reload persisted installs and start their background threads.
fn restore_extensions(app: &AppHandle) {
    let state = app.state::<BrowserState>();
    let roots: Vec<String> = {
        let db = lock(&state.db);
        match db.ext_kv_get("sys", "installed") {
            Ok(Some(json)) => serde_json::from_str(&json).unwrap_or_default(),
            _ => Vec::new(),
        }
    };
    // Remembered API grants first: threads read them at startup.
    let grants: Vec<(String, Vec<String>)> = {
        let db = lock(&state.db);
        match db.ext_kv_get("sys", "api_grants") {
            Ok(Some(json)) => serde_json::from_str(&json).unwrap_or_default(),
            _ => Vec::new(),
        }
    };
    {
        let mut pm = lock(&state.permission);
        for (ext, apis) in grants {
            let id = surf_ext_host::ExtensionId(ext);
            for api in apis {
                pm.store_mut().grant_api(&id, &api);
            }
        }
    }
    for root in roots {
        let mut registry = lock(&state.registry);
        if let Ok(id) = registry.install_dir(&root) {
            let ext = registry.get(&id).cloned();
            drop(registry);
            if let Some(ext) = ext {
                lock(&state.bg_threads).start(app, ext);
            }
        } else {
            eprintln!("saved extension root missing, skipping: {root}");
        }
    }
}

/// Persist the install list and API grants.
pub fn persist_extension_state(app: &AppHandle) {
    let state = app.state::<BrowserState>();
    let roots: Vec<String> = lock(&state.registry)
        .list()
        .into_iter()
        .map(|e| e.root.to_string_lossy().into_owned())
        .collect();
    let grants: Vec<(String, Vec<String>)> = state
        .permission
        .lock()
        .unwrap()
        .store()
        .api_grants()
        .into_iter()
        .collect();
    let db = lock(&state.db);
    let _ = db.ext_kv_set("sys", "installed", &serde_json::to_string(&roots).unwrap_or_default());
    let _ = db.ext_kv_set("sys", "api_grants", &serde_json::to_string(&grants).unwrap_or_default());
}

pub fn notify_bg(app: &AppHandle, id: &surf_ext_host::ExtensionId, msg: BgMsg) {
    let state = app.state::<BrowserState>();
    let tx = lock(&state.bg_threads).sender(id);
    if let Some(tx) = tx {
        let _ = tx.send(msg);
    }
}

/// Alarm wakeups: a single daemon thread checks due alarms once a second
/// and delivers an `alarm` event into the owning extension's thread.
fn start_alarm_scheduler(app: &AppHandle) {
    let handle = app.clone();
    std::thread::Builder::new()
        .name("alarm-scheduler".into())
        .spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let state = handle.state::<BrowserState>();
            let due: Vec<(String, String)> = {
                let mut alarms = lock(&state.alarms);
                let now = crate::api_impl::now_ms();
                let mut due = Vec::new();
                for (key, st) in alarms.iter_mut() {
                    if st.at_ms <= now {
                        due.push((key.0.clone(), key.1.clone()));
                        if let Some(period) = st.period_ms {
                            st.at_ms = now + period;
                        }
                    }
                }
                alarms.retain(|_, st| st.at_ms > now || st.period_ms.is_some());
                due
            };
            for (ext, name) in due {
                notify_bg(
                    &handle,
                    &surf_ext_host::ExtensionId(ext.clone()),
                    BgMsg::Envelope(surf_ext_host::router::Envelope {
                        from: surf_ext_host::router::Endpoint::Host,
                        to: surf_ext_host::router::Endpoint::Extension(surf_ext_host::ExtensionId(ext)),
                        body: surf_ext_host::router::MessageBody::Event {
                            name: "alarm".into(),
                            payload: serde_json::json!({ "name": name }),
                        },
                    }),
                );
            }
        })
        .expect("spawn alarm scheduler");
}

/// Reopen the persisted session. Fresh tab ids are fine: nothing outside
/// this process references them.
fn restore_session(app: &AppHandle) {
    let state = app.state::<BrowserState>();
    let saved = { lock(&state.db).load_tabs().unwrap_or_default() };
    #[cfg(desktop)]
    {
        let window = app.get_window(MAIN_LABEL).expect("main window exists");
        if saved.is_empty() {
            crate::manager::open_tab(app, &window, None);
            return;
        }
        let mut id_by_position: HashMap<usize, TabId> = HashMap::new();
        for (position, tab) in saved.into_iter().enumerate() {
            let id = crate::manager::open_tab(app, &window, tab.url.clone());
            if let Some(title) = tab.title {
                lock(&state.model).set_title(id, title);
            }
            id_by_position.insert(position, id);
        }
        // A split view persisted before quit returns with the session.
        if crate::manager::restore_split(&state, &id_by_position).is_some() {
            let _ = layout::relayout(app);
        }
    }
    #[cfg(mobile)]
    {
        // Same as desktop now: every restored tab gets a child webview;
        // open_tab activates the last one and the layout pass stacks it.
        let window = app.get_window(MAIN_LABEL).expect("main window exists");
        if saved.is_empty() {
            crate::manager::open_tab(app, &window, None);
        } else {
            for tab in saved {
                let id = crate::manager::open_tab(app, &window, tab.url.clone());
                if let Some(title) = tab.title {
                    lock(&state.model).set_title(id, title);
                }
            }
        }
        let _ = layout::relayout(app);
    }
    // open_tab leaves the last one active, matching persisted order.
    crate::manager::emit_tabs(app);
}

fn watch_resize(app: &AppHandle, window: &Window<Wry>) {
    let handle = app.clone();
    let win = window.clone();
    window.on_window_event(move |event| match event {
        tauri::WindowEvent::Resized(_) => {
            let _ = layout::relayout(&handle);
        }
        // Moving between displays with different scale factors changes
        // every physical geometry; recompute with the new scale.
        tauri::WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
            let _ = new_inner_size;
            let _ = layout::relayout_window(&win);
        }
        tauri::WindowEvent::Moved(_) => {
            let _ = layout::relayout(&handle);
        }
        _ => {}
    });
}

/// Cheap clone of a tab's webview handle, wrapped as a TabView.
pub fn content_webview(app: &AppHandle, id: TabId) -> Option<TauriTabView> {
    let state = app.state::<BrowserState>();
    let view = lock(&state.webviews).get(&id).map(|v| TauriTabView(v.raw().clone()));
    view
}

pub fn take_content_webview(app: &AppHandle, id: TabId) -> Option<TauriTabView> {
    let state = app.state::<BrowserState>();
    let view = lock(&state.webviews).remove(&id);
    view
}

pub fn home_url() -> Url {
    Url::parse("https://duckduckgo.com/").unwrap()
}
