mod api_impl;
mod bg;
mod commands;
mod content_bridge;
mod deep_links;
mod ext_scheme;
mod filters;
mod keychain;
mod layout;
mod manager;
mod nav_filter;
mod menus;
mod state;
mod tabview;

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
            app.manage(state::BrowserState::new(db));

            state::setup(app)
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_tabs,
            commands::tab_open,
            commands::tab_close,
            commands::tab_activate,
            commands::tab_navigate,
            commands::tab_back,
            commands::tab_forward,
            commands::tab_reload,
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
            commands::settings_get,
            commands::settings_set,
            commands::rules_get,
            commands::rules_set,
            commands::sync_export,
            commands::sync_import,
        ])
        .run(tauri::generate_context!())
        .expect("tauri failed to start");
}
