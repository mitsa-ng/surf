use surf_core::tabs::TabId;
use tauri::AppHandle;

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
    #[cfg(mobile)]
    {
        let _ = (&app, &url);
        return Err(crate::CommandError::Message("行動端內容分頁在 M2 啟用".into()));
    }
    #[cfg(desktop)]
    {
        let parsed = url
            .map(|u| url::Url::parse(&u).map_err(|e| crate::CommandError::Message(e.to_string())))
            .transpose()?;
        let window = app
            .get_window(state::MAIN_LABEL)
            .ok_or_else(|| crate::CommandError::Message("main window missing".into()))?;
        crate::manager::open_tab(&app, &window, parsed);
        Ok(snapshot(&app))
    }
}

#[tauri::command]
pub fn tab_close(app: AppHandle, id: u64) -> Result<TabsSnapshot, crate::CommandError> {
    #[cfg(mobile)]
    {
        let _ = &app;
        return Err(crate::CommandError::Message("行動端內容分頁在 M2 啟用".into()));
    }
    #[cfg(desktop)]
    {
        crate::manager::close_tab(&app, TabId(id))?;
        Ok(snapshot(&app))
    }
}

#[tauri::command]
pub fn tab_activate(app: AppHandle, id: u64) -> Result<TabsSnapshot, crate::CommandError> {
    #[cfg(mobile)]
    {
        let _ = &app;
        return Err(crate::CommandError::Message("行動端內容分頁在 M2 啟用".into()));
    }
    #[cfg(desktop)]
    {
        crate::manager::activate_tab(&app, TabId(id))?;
        Ok(snapshot(&app))
    }
}

#[tauri::command]
pub fn tab_navigate(app: AppHandle, id: u64, input: String) -> Result<TabsSnapshot, crate::CommandError> {
    #[cfg(mobile)]
    {
        let _ = (&app, &input);
        return Err(crate::CommandError::Message("行動端內容分頁在 M2 啟用".into()));
    }
    #[cfg(desktop)]
    {
        crate::manager::navigate(&app, TabId(id), &input)?;
        Ok(snapshot(&app))
    }
}

#[tauri::command]
pub fn tab_back(app: AppHandle, id: u64) -> Result<(), crate::CommandError> {
    #[cfg(mobile)]
    {
        let _ = &app;
        return Err(crate::CommandError::Message("行動端內容分頁在 M2 啟用".into()));
    }
    #[cfg(desktop)]
    {
        crate::manager::eval_history(&app, TabId(id), "history.back()")
    }
}

#[tauri::command]
pub fn tab_forward(app: AppHandle, id: u64) -> Result<(), crate::CommandError> {
    #[cfg(mobile)]
    {
        let _ = &app;
        return Err(crate::CommandError::Message("行動端內容分頁在 M2 啟用".into()));
    }
    #[cfg(desktop)]
    {
        crate::manager::eval_history(&app, TabId(id), "history.forward()")
    }
}

#[tauri::command]
pub fn tab_reload(app: AppHandle, id: u64) -> Result<(), crate::CommandError> {
    #[cfg(mobile)]
    {
        let _ = &app;
        return Err(crate::CommandError::Message("行動端內容分頁在 M2 啟用".into()));
    }
    #[cfg(desktop)]
    {
        crate::manager::eval_history(&app, TabId(id), "location.reload()")
    }
}

#[tauri::command]
pub fn filter_capabilities() -> surf_ext_host::rules::FilterCapabilities {
    crate::filters::platform_adapter().capabilities()
}
