mod agent;
mod agui;
mod api_impl;
mod app_icon;
#[cfg(desktop)]
mod auto_sync;
mod bg;
mod commands;
mod content_bridge;
mod deep_links;
mod ext_scheme;
mod favicon;
#[cfg(target_os = "android")]
mod file_secrets;
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
            #[cfg(target_os = "android")]
            let secrets_dir = Some(data_dir.join("secrets"));
            #[cfg(not(target_os = "android"))]
            let secrets_dir = None;
            app.manage(state::BrowserState::new(db, favicon_dir, secrets_dir));

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
            layout::chrome_metrics_set,
            layout::bookmarks_bar_set_open,
            layout::split_set,
            layout::split_clear,
            commands::history_clear,
            commands::downloads_list,
            commands::download_reveal,
            commands::find_in_page,
            commands::suggest,
            favicon::favicon_url,
            agui::agent_chat_set_open,
            agui::agent_chat_send,
            agui::agent_chat_history,
            agui::agent_chat_reset,
            agui::agent_agui_config_get,
            agui::agent_agui_config_set,
            app_icon::app_icon_get,
            app_icon::app_icon_set,
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

#[cfg(test)]
mod tests {
    /// The AG-UI round's silent-failure bug: commands registered in
    /// `generate_handler!` but missing from every permission manifest
    /// are denied at the ACL with no visible error — the chrome
    /// invokes, the invoke dies, nothing logs. This test fails the
    /// build the moment a new command forgets its allow entry.
    #[test]
    fn every_registered_command_is_allowed_by_some_permission() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let lib_rs = std::fs::read_to_string(format!("{}/src/lib.rs", manifest_dir)).expect("read lib.rs");

        // Collect `module::command` identifiers inside the
        // generate_handler![ ... ] block.
        let start = lib_rs
            .find("generate_handler![")
            .map(|i| lib_rs[i..].find('[').map(|j| i + j + 1).unwrap_or(i))
            .expect("generate_handler! block");
        let end = lib_rs[start..].find(']').map(|i| start + i).expect("handler block closes");
        let handler_block = &lib_rs[start..end];
        let registered: Vec<String> = handler_block
            .lines()
            .filter_map(|l| l.split("//").next())
            .filter_map(|l| l.trim().strip_suffix(','))
            .filter(|l| l.contains("::"))
            .map(|l| l.rsplit("::").next().unwrap_or(l).trim().to_owned())
            .filter(|l| !l.is_empty())
            .collect();
        assert!(!registered.is_empty(), "no commands parsed from generate_handler!");

        // Collect every quoted token inside commands.allow = [ ... ]
        // lists across the permission manifests.
        let perms_dir = format!("{}/permissions", manifest_dir);
        let mut allowed: Vec<String> = Vec::new();
        for entry in std::fs::read_dir(&perms_dir).expect("permissions dir") {
            let toml = std::fs::read_to_string(entry.expect("dir entry").path()).expect("read toml");
            let mut in_allow = false;
            for line in toml.lines() {
                if line.contains("commands.allow") {
                    in_allow = true;
                }
                if in_allow {
                    allowed.extend(
                        line.split('"')
                            .skip(1)
                            .step_by(2)
                            .map(str::to_owned)
                            .collect::<Vec<_>>(),
                    );
                    if line.contains(']') {
                        in_allow = false;
                    }
                }
            }
        }
        assert!(!allowed.is_empty(), "no commands.allow parsed from permissions/");

        let missing: Vec<&String> = registered.iter().filter(|c| !allowed.contains(c)).collect();
        assert!(
            missing.is_empty(),
            "commands registered in generate_handler! but allowed by no permissions/*.toml \
             (the ACL denies them silently): {missing:?}\nfix: add them to \
             permissions/app-commands.toml"
        );
    }
}
