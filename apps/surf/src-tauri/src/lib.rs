mod commands;
mod filters;
mod layout;
mod manager;
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
        ])
        .run(tauri::generate_context!())
        .expect("tauri failed to start");
}
