mod agent;
mod api_impl;
#[cfg(desktop)]
mod auto_sync;
mod bg;
mod commands;
mod content_bridge;
mod deep_links;
mod ext_scheme;
mod favicon;
mod filters;
mod keychain;
mod layout;
mod manager;
mod nav_filter;
mod menus;
mod state;
mod tabview;
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod wk_rules;

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("{0}")]
    Message(String),
    #[error("unknown tab: {0}")]
    UnknownTab(u64),
}

impl serde::Serialize for CommandError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .register_uri_scheme_protocol("ext", ext_scheme::handle)
        .register_uri_scheme_protocol("favicon", favicon::handle_asset)
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            use tauri::Manager;

            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db = surf_core::storage::Db::open(data_dir.join("surf.db")).unwrap_or_else(|e| {
                eprintln!("storage open failed, falling back to in-memory: {e}");
                surf_core::storage::Db::open(":memory:").expect("in-memory db")
            });
            let favicon_dir = app
                .path()
                .app_cache_dir()
                .map(|d| d.join("favicons"))
                .unwrap_or_else(|_| data_dir.join("favicons"));
            app.manage(state::BrowserState::new(db, favicon_dir));

            state::setup(app)
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_tabs,
            commands::tab_open,
            commands::tab_open_incognito,
            commands::tab_set_pinned,
            commands::tab_reopen_closed,
            commands::tab_close,
            commands::tab_close_others,
            commands::tab_close_right,
            commands::tab_activate,
            commands::tab_navigate,
            commands::tab_back,
            commands::tab_forward,
            commands::tab_reload,
            commands::tab_stop,
            commands::tab_zoom,
            commands::tab_set_muted,
            commands::deeplink_take,
            commands::filter_capabilities,
            commands::extensions_list,
            commands::extension_install,
            commands::extension_uninstall,
            commands::extension_grant,
            commands::ext_popup_open,
            commands::ext_options_open,
            commands::ext_api_call,
            commands::cs_bootstrap,
            commands::cs_send,
            commands::bookmarks_list,
            commands::bookmarks_add,
            commands::bookmarks_remove,
            commands::history_recent,
            commands::history_delete,
            commands::history_search,
            commands::settings_get,
            commands::settings_set,
            commands::rules_get,
            commands::rules_set,
            commands::sync_export,
            commands::sync_import,
            commands::sync_auto_get,
            commands::sync_auto_set,
            commands::pick_sync_file,
            layout::panel_set_open,
            layout::overlay_set_open,
            agent::agent_panel_set_open,
            layout::bookmarks_bar_set_open,
            layout::split_set,
            layout::split_clear,
            commands::history_clear,
            commands::downloads_list,
            commands::download_reveal,
            commands::find_in_page,
            commands::suggest,
            favicon::favicon_url,
            agent::agent_action,
            agent::agent_llm_config_get,
            agent::agent_llm_config_set,
            commands::password_save,
            commands::password_dismiss,
            commands::password_list,
            commands::password_delete,
        ])
        .run(tauri::generate_context!())
        .expect("tauri failed to start");
}
