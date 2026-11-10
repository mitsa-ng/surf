use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use surf_core::storage::Db;
use surf_core::tabs::{TabId, TabModel, TabState};
use surf_ext_host::permission::PermissionManager;
use surf_ext_host::planner::InjectionPlanner;
use surf_ext_host::router::Router;
use surf_ext_host::Registry;
use tauri::{AppHandle, Manager, Window, WindowBuilder, Wry};
use url::Url;

use crate::api_impl::AlarmState;
use crate::bg::{BgMsg, BgRegistry};
use crate::content_bridge::TokenRegistry;
use crate::layout;
use crate::tabview::TauriTabView;

#[derive(serde::Serialize, Clone)]
pub struct TabsSnapshot {
    pub tabs: Vec<TabState>,
    pub active: Option<TabId>,
}

pub struct BrowserState {
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
}

impl BrowserState {
    pub fn new(db: Db) -> Self {
        Self {
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
            #[cfg(not(target_os = "macos"))]
            secrets: std::sync::Arc::new(surf_core::crypto::MemorySecretStore::new()),
            nav_filter: crate::nav_filter::NavFilter::default(),
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
        let window = WindowBuilder::new(app, MAIN_LABEL)
            .title("Surf")
            .inner_size(1200.0, 800.0)
            .min_inner_size(480.0, 480.0)
            .build()?;

        let scale = window.scale_factor().unwrap_or(1.0);
        let size = window.inner_size()?;
        let offset = layout::titlebar_offset(&window);
        let chrome = window.add_child(
            tauri::webview::WebviewBuilder::new("chrome", tauri::WebviewUrl::App("index.html".into()))
                .focused(true),
            tauri::LogicalPosition::new(0.0, offset),
            tauri::LogicalSize::new(size.width as f64 / scale, layout::CHROME_HEIGHT),
        )?;
        drop(chrome);

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
        drop(win);
    }

    crate::deep_links::wire(&handle);
    {
        let state = app.state::<BrowserState>();
        let filters = crate::nav_filter::load_filters(&lock(&state.db));
        if !filters.is_empty() {
            state.nav_filter.start(filters);
        }
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
        for tab in saved {
            let id = crate::manager::open_tab(app, &window, tab.url.clone());
            if let Some(title) = tab.title {
                lock(&state.model).set_title(id, title);
            }
        }
    }
    #[cfg(mobile)]
    {
        // Model entries for every saved tab; only the active one gets a
        // webview window now, the rest are created lazily on activate.
        let mut last: Option<surf_core::tabs::TabId> = None;
        for tab in saved {
            let id = crate::manager::model_open(app, tab.url.clone());
            if let Some(title) = tab.title {
                lock(&state.model).set_title(id, title);
            }
            last = Some(id);
        }
        if let Some(id) = last {
            let url = lock(&state.model).get(id).and_then(|t| t.url.clone());
            let _ = crate::manager::ensure_tab_window(app, id, url);
        }
    }
    // open_tab leaves the last one active, matching persisted order.
    crate::manager::emit_tabs(app);
}

#[cfg(desktop)]
fn watch_resize(app: &AppHandle, window: &Window<Wry>) {
    let handle = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Resized(_) = event {
            let _ = layout::relayout(&handle);
        }
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
