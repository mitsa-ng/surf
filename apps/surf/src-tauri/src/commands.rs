use surf_core::tabs::TabId;
use tauri::{AppHandle, Manager};

use crate::state::{self, BrowserState, TabsSnapshot};


#[tauri::command]
pub fn get_tabs(app: AppHandle) -> TabsSnapshot {
    snapshot(&app)
}

fn snapshot(app: &AppHandle) -> TabsSnapshot {
    let state = app.state::<BrowserState>();
    let model = state::lock(&state.model);
    TabsSnapshot { tabs: model.list().to_vec(), active: model.active_id() }
}

#[tauri::command]
pub fn tab_open(app: AppHandle, url: Option<String>) -> Result<TabsSnapshot, crate::CommandError> {
    let parsed = url
        .map(|u| url::Url::parse(&u).map_err(|e| crate::CommandError::Message(e.to_string())))
        .transpose()?;
    #[cfg(desktop)]
    {
        let window = app
            .get_window(state::MAIN_LABEL)
            .ok_or_else(|| crate::CommandError::Message("main window missing".into()))?;
        crate::manager::open_tab(&app, &window, parsed);
    }
    #[cfg(mobile)]
    {
        crate::manager::open_tab_mobile(&app, parsed);
    }
    Ok(snapshot(&app))
}

#[tauri::command]
pub fn tab_close(app: AppHandle, id: u64) -> Result<TabsSnapshot, crate::CommandError> {
    #[cfg(desktop)]
    {
        crate::manager::close_tab(&app, TabId(id))?;
    }
    #[cfg(mobile)]
    {
        crate::manager::close_tab_mobile(&app, TabId(id)).map_err(|e| crate::CommandError::Message(e.to_string()))?;
    }
    Ok(snapshot(&app))
}

#[tauri::command]
pub fn tab_activate(app: AppHandle, id: u64) -> Result<TabsSnapshot, crate::CommandError> {
    #[cfg(desktop)]
    {
        crate::manager::activate_tab(&app, TabId(id))?;
    }
    #[cfg(mobile)]
    {
        crate::manager::activate_tab_mobile(&app, TabId(id)).map_err(|e| crate::CommandError::Message(e.to_string()))?;
    }
    Ok(snapshot(&app))
}

#[tauri::command]
pub fn tab_navigate(app: AppHandle, id: u64, input: String) -> Result<TabsSnapshot, crate::CommandError> {
    #[cfg(desktop)]
    {
        crate::manager::navigate(&app, TabId(id), &input)?;
    }
    #[cfg(mobile)]
    {
        crate::manager::navigate_mobile(&app, TabId(id), &input).map_err(|e| crate::CommandError::Message(e.to_string()))?;
    }
    Ok(snapshot(&app))
}

#[tauri::command]
pub fn tab_back(app: AppHandle, id: u64) -> Result<(), crate::CommandError> {
    #[cfg(desktop)]
    {
        crate::manager::eval_history(&app, TabId(id), "history.back()")?;
    }
    #[cfg(mobile)]
    {
        // Per-tab webview history arrives with instance-swap (M3); the
        // model-level reload hints the UI instead.
        let _ = (&app, &id);
    }
    Ok(())
}

#[tauri::command]
pub fn tab_forward(app: AppHandle, id: u64) -> Result<(), crate::CommandError> {
    #[cfg(desktop)]
    {
        crate::manager::eval_history(&app, TabId(id), "history.forward()")?;
    }
    #[cfg(mobile)]
    {
        // Per-tab webview history arrives with instance-swap (M3); the
        // model-level reload hints the UI instead.
        let _ = (&app, &id);
    }
    Ok(())
}

#[tauri::command]
pub fn tab_reload(app: AppHandle, id: u64) -> Result<(), crate::CommandError> {
    #[cfg(desktop)]
    {
        crate::manager::eval_history(&app, TabId(id), "location.reload()")?;
    }
    #[cfg(mobile)]
    {
        // Per-tab webview history arrives with instance-swap (M3); the
        // model-level reload hints the UI instead.
        let _ = (&app, &id);
    }
    Ok(())
}

#[tauri::command]
pub fn filter_capabilities() -> surf_ext_host::rules::FilterCapabilities {
    crate::filters::platform_adapter().capabilities()
}

#[derive(serde::Serialize)]
pub struct ExtensionSummary {
    pub id: String,
    pub name: String,
    pub version: String,
}

#[derive(serde::Serialize)]
pub struct BookmarkRow {
    pub url: String,
    pub title: Option<String>,
}

#[tauri::command]
pub fn extensions_list(app: AppHandle) -> Vec<ExtensionSummary> {
    let state = app.state::<BrowserState>();
    let registry = state::lock(&state.registry);
    registry
        .list()
        .into_iter()
        .map(|ext| ExtensionSummary {
            id: ext.id.0.clone(),
            name: ext.manifest.name.clone(),
            version: ext.manifest.version.clone(),
        })
        .collect()
}

#[tauri::command]
pub fn extension_install(app: AppHandle, path: String) -> Result<ExtensionSummary, crate::CommandError> {
    let state = app.state::<BrowserState>();
    let id = {
        let mut registry = state::lock(&state.registry);
        let id = registry
            .install_dir(&path)
            .map_err(|e| crate::CommandError::Message(e.to_string()))?;
        let ext = registry.get(&id).cloned().expect("just installed");
        // Grant the manifest's declared API permissions via the prompt
        // flow: for now the install dialog in the UI collects consent and
        // calls grant command; here we only register.
        drop(registry);
        state::lock(&state.bg_threads).start(&app, ext);
        id
    };
    state::persist_extension_state(&app);
    let ext = state::lock(&state.registry).get(&id).expect("just installed").clone();
    Ok(ExtensionSummary { id: id.0, name: ext.manifest.name, version: ext.manifest.version })
}

#[tauri::command]
pub fn extension_uninstall(app: AppHandle, id: String) -> Result<Vec<ExtensionSummary>, crate::CommandError> {
    let state = app.state::<BrowserState>();
    let ext_id = surf_ext_host::ExtensionId(id);
    state::lock(&state.registry)
        .uninstall(&ext_id)
        .map_err(|e| crate::CommandError::Message(e.to_string()))?;
    state::persist_extension_state(&app);
    Ok(extensions_list(app))
}

#[tauri::command]
pub fn extension_grant(app: AppHandle, id: String, apis: Vec<String>) -> Result<(), crate::CommandError> {
    let state = app.state::<BrowserState>();
    let ext_id = surf_ext_host::ExtensionId(id);
    {
        let mut pm = state::lock(&state.permission);
        for api in &apis {
            pm.store_mut().grant_api(&ext_id, api);
        }
    }
    state::persist_extension_state(&app);
    Ok(())
}

/// Open the extension's popup as a small chrome window served from ext://.
#[tauri::command]
pub fn ext_popup_open(app: AppHandle, id: String) -> Result<(), crate::CommandError> {
    let state = app.state::<BrowserState>();
    let ext_id = surf_ext_host::ExtensionId(id.clone());
    let popup_path = {
        let registry = state::lock(&state.registry);
        let ext = registry
            .get(&ext_id)
            .ok_or_else(|| crate::CommandError::Message("unknown extension".into()))?;
        ext.manifest
            .action
            .as_ref()
            .and_then(|a| a.popup.clone())
            .unwrap_or_else(|| "popup.html".to_owned())
    };
    let label = format!("ext-popup-{id}");
    if app.get_webview_window(&label).is_some() {
        return Ok(());
    }
    let url = tauri::WebviewUrl::External(
        url::Url::parse(&format!("ext://{id}/{popup_path}")).map_err(|e| crate::CommandError::Message(e.to_string()))?,
    );
    tauri::WebviewWindowBuilder::new(&app, label, url)
        .title("Surf 擴充功能")
        .inner_size(360.0, 480.0)
        .build()
        .map_err(|e| crate::CommandError::Message(e.to_string()))?;
    Ok(())
}

/// Extension pages (popup/options) call host APIs through this command;
/// the extension id comes from the window label so the page cannot claim
/// another extension's identity.
#[tauri::command]
pub fn ext_api_call(
    app: AppHandle,
    window: tauri::WebviewWindow,
    module: String,
    method: String,
    args: serde_json::Value,
) -> Result<serde_json::Value, crate::CommandError> {
    let label = window.label().to_owned();
    let id = label
        .strip_prefix("ext-popup-")
        .ok_or_else(|| crate::CommandError::Message("ext_api_call is for extension pages only".into()))?
        .to_owned();
    let state = app.state::<BrowserState>();
    let ext_id = surf_ext_host::ExtensionId(id);
    {
        let mut pm = state::lock(&state.permission);
        if !pm.ensure(&ext_id, &surf_ext_host::permission::PermissionRequest::Api(module.clone())) {
            return Err(crate::CommandError::Message(format!("permission denied for {module}")));
        }
    }
    let ctx = surf_ext_host::api::ExtContext { ext: ext_id, tab: None };
    let host = crate::api_impl::build_host(app.clone());
    host.call(&ctx, &module, &method, &args)
        .map_err(|e| crate::CommandError::Message(e.to_string()))
}

#[tauri::command]
pub fn bookmarks_list(app: AppHandle) -> Result<Vec<BookmarkRow>, crate::CommandError> {
    let state = app.state::<BrowserState>();
    let rows = state::lock(&state.db)
        .bookmarks()
        .map_err(|e| crate::CommandError::Message(e.to_string()))?
        .into_iter()
        .map(|(u, t)| BookmarkRow { url: u.to_string(), title: t })
        .collect();
    Ok(rows)
}

#[tauri::command]
pub fn bookmarks_add(app: AppHandle, url: String, title: Option<String>) -> Result<Vec<BookmarkRow>, crate::CommandError> {
    let state = app.state::<BrowserState>();
    let parsed = url::Url::parse(&url).map_err(|e| crate::CommandError::Message(e.to_string()))?;
    state::lock(&state.db)
        .add_bookmark(&parsed, title.as_deref())
        .map_err(|e| crate::CommandError::Message(e.to_string()))?;
    bookmarks_list(app)
}

#[tauri::command]
pub fn bookmarks_remove(app: AppHandle, url: String) -> Result<Vec<BookmarkRow>, crate::CommandError> {
    let state = app.state::<BrowserState>();
    let parsed = url::Url::parse(&url).map_err(|e| crate::CommandError::Message(e.to_string()))?;
    state::lock(&state.db)
        .remove_bookmark(&parsed)
        .map_err(|e| crate::CommandError::Message(e.to_string()))?;
    bookmarks_list(app)
}

#[derive(serde::Serialize)]
pub struct HistoryRow {
    pub url: String,
    pub title: Option<String>,
    pub visited_at: i64,
}

#[tauri::command]
pub fn history_recent(app: AppHandle, limit: Option<usize>) -> Result<Vec<HistoryRow>, crate::CommandError> {
    let state = app.state::<BrowserState>();
    let rows = state::lock(&state.db)
        .history_recent(limit.unwrap_or(100))
        .map_err(|e| crate::CommandError::Message(e.to_string()))?
        .into_iter()
        .map(|(u, t, ts)| HistoryRow { url: u.to_string(), title: t, visited_at: ts })
        .collect();
    Ok(rows)
}

#[tauri::command]
pub fn cs_bootstrap(
    app: AppHandle,
    token: String,
    url: String,
) -> Result<serde_json::Value, crate::CommandError> {
    let state = app.state::<BrowserState>();
    let id = state
        .cs_tokens
        .resolve(&token)
        .ok_or_else(|| crate::CommandError::Message("unknown content token".into()))?;
    let target = url::Url::parse(&url).map_err(|e| crate::CommandError::Message(e.to_string()))?;

    let registry = state::lock(&state.registry);
    let installed: Vec<&surf_ext_host::InstalledExtension> = registry.list().into_iter().collect();
    let plan = state::lock(&state.planner).plan(&installed, &target);

    let mut scripts = Vec::new();
    for item in plan {
        let ext = match registry.get(&item.ext) {
            Some(e) => e,
            None => continue,
        };
        let path = ext.root.join(&item.script);
        match std::fs::read_to_string(&path) {
            Ok(code) => scripts.push(serde_json::json!({
                "ext": item.ext.0,
                "runAt": match item.run_at {
                    surf_ext_host::manifest::RunAt::DocumentStart => "documentstart",
                    surf_ext_host::manifest::RunAt::DocumentEnd => "documentend",
                    surf_ext_host::manifest::RunAt::DocumentIdle => "documentidle",
                },
                "code": code,
            })),
            Err(e) => eprintln!("content script {} unreadable: {e}", item.script),
        }
    }
    Ok(serde_json::json!({ "tab": id.0, "scripts": scripts }))
}

#[tauri::command]
pub fn cs_send(
    app: AppHandle,
    token: String,
    name: String,
    payload: serde_json::Value,
    ext: Option<String>,
) -> Result<(), crate::CommandError> {
    let state = app.state::<BrowserState>();
    let id = state
        .cs_tokens
        .resolve(&token)
        .ok_or_else(|| crate::CommandError::Message("unknown content token".into()))?;
    let to = match ext {
        Some(e) => surf_ext_host::router::Endpoint::Extension(surf_ext_host::ExtensionId(e)),
        None => surf_ext_host::router::Endpoint::Host,
    };
    let envelope = surf_ext_host::router::Envelope {
        from: surf_ext_host::router::Endpoint::Content(id),
        to,
        body: surf_ext_host::router::MessageBody::Event { name, payload },
    };
    let router = state::lock(&state.router);
    router.send(envelope).map_err(|e| crate::CommandError::Message(e.to_string()))
}

#[tauri::command]
pub fn settings_get(app: AppHandle, key: String) -> Result<Option<String>, crate::CommandError> {
    let state = app.state::<BrowserState>();
    let value = state::lock(&state.db)
        .ext_kv_get("sys", &format!("setting:{key}"))
        .map_err(|e| crate::CommandError::Message(e.to_string()))?;
    Ok(value)
}

#[tauri::command]
pub fn settings_set(app: AppHandle, key: String, value: String) -> Result<(), crate::CommandError> {
    let state = app.state::<BrowserState>();
    let result = state::lock(&state.db)
        .ext_kv_set("sys", &format!("setting:{key}"), &value)
        .map_err(|e| crate::CommandError::Message(e.to_string()));
    result
}

#[tauri::command]
pub fn rules_get(app: AppHandle) -> Result<Vec<String>, crate::CommandError> {
    let state = app.state::<BrowserState>();
    let filters = crate::nav_filter::load_filters(&state::lock(&state.db));
    Ok(filters)
}

#[tauri::command]
pub fn rules_set(app: AppHandle, filters: Vec<String>) -> Result<Vec<String>, crate::CommandError> {
    let state = app.state::<BrowserState>();
    let save = crate::nav_filter::save_filters(&state::lock(&state.db), &filters)
        .map_err(|e| crate::CommandError::Message(e.to_string()));
    save?;
    if !filters.is_empty() {
        state.nav_filter.start(filters);
    } else {
        // Empty list: drop the engine so everything is allowed.
        state.nav_filter.start(vec!["@@*".into()]);
    }
    let saved = crate::nav_filter::load_filters(&state::lock(&state.db));
    Ok(saved)
}
