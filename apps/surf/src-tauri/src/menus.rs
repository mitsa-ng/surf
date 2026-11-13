//! Native application menus (desktop). Menu items emit `menu://action`
//! events to the chrome webview; the UI owns the resulting behavior so
//! there is exactly one action path.

#[cfg(desktop)]
pub fn build(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};

    let new_tab = MenuItemBuilder::with_id("surf.new-tab", "新分頁").accelerator("CmdOrCtrl+T").build(app)?;
    let new_private = MenuItemBuilder::with_id("surf.new-private", "開新無痕分頁").accelerator("CmdOrCtrl+Shift+N").build(app)?;
    let reopen = MenuItemBuilder::with_id("surf.reopen-tab", "重開已關閉分頁").accelerator("CmdOrCtrl+Shift+T").build(app)?;
    let close_tab = MenuItemBuilder::with_id("surf.close-tab", "關閉分頁").accelerator("CmdOrCtrl+W").build(app)?;
    let downloads = MenuItemBuilder::with_id("surf.downloads", "下載項目").accelerator("CmdOrCtrl+Shift+J").build(app)?;
    let bookmarks = MenuItemBuilder::with_id("surf.bookmarks", "書籤面板").build(app)?;
    let history = MenuItemBuilder::with_id("surf.history", "瀏覽紀錄").build(app)?;
    let extensions = MenuItemBuilder::with_id("surf.extensions", "擴充功能").build(app)?;
    let settings = MenuItemBuilder::with_id("surf.settings", "設定").build(app)?;

    let file = SubmenuBuilder::new(app, "檔案")
        .item(&new_tab)
        .item(&new_private)
        .item(&reopen)
        .item(&close_tab)
        .separator()
        .item(&downloads)
        .separator()
        .quit()
        .build()?;
    let edit = SubmenuBuilder::new(app, "編輯")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .separator()
        .item(&MenuItemBuilder::with_id("surf.find", "尋找…").accelerator("CmdOrCtrl+F").build(app)?)
        .build()?;
    let reload = MenuItemBuilder::with_id("surf.reload", "重新載入").accelerator("CmdOrCtrl+R").build(app)?;
    let tab_search = MenuItemBuilder::with_id("surf.tab-search", "搜尋分頁").accelerator("CmdOrCtrl+Shift+A").build(app)?;
    let bookmarks_bar = MenuItemBuilder::with_id("surf.bookmarks-bar", "書籤列").accelerator("CmdOrCtrl+Shift+B").build(app)?;
    let zoom_in = MenuItemBuilder::with_id("surf.zoom-in", "放大").accelerator("CmdOrCtrl+Plus").build(app)?;
    let zoom_out = MenuItemBuilder::with_id("surf.zoom-out", "縮小").accelerator("CmdOrCtrl+Minus").build(app)?;
    let zoom_reset = MenuItemBuilder::with_id("surf.zoom-reset", "實際大小").accelerator("CmdOrCtrl+0").build(app)?;
    let view = SubmenuBuilder::new(app, "檢視")
        .item(&reload)
        .separator()
        .item(&tab_search)
        .item(&bookmarks_bar)
        .separator()
        .item(&zoom_in)
        .item(&zoom_out)
        .item(&zoom_reset)
        .separator()
        .fullscreen()
        .build()?;
    let surf_menu = SubmenuBuilder::new(app, "瀏覽")
        .item(&bookmarks)
        .item(&history)
        .item(&extensions)
        .item(&settings)
        .build()?;
    let shortcuts = MenuItemBuilder::with_id("surf.shortcuts", "鍵盤快速鍵").build(app)?;
    let about = MenuItemBuilder::with_id("surf.about", "關於 Surf").build(app)?;
    let help = SubmenuBuilder::new(app, "說明")
        .item(&shortcuts)
        .item(&about)
        .build()?;

    let menu = MenuBuilder::new(app)
        .item(&file)
        .item(&edit)
        .item(&view)
        .item(&surf_menu)
        .item(&help)
        .build()?;
    let _ = PredefinedMenuItem::separator(app)?;
    app.set_menu(menu)?;
    Ok(())
}

/// Menu events funnel here; everything is forwarded as one event kind so
/// the UI dispatch stays single-source.
#[cfg(desktop)]
pub fn wire_events(app: &tauri::AppHandle) {
    use tauri::{Emitter, Manager};
    let handle = app.clone();
    app.on_menu_event(move |_app, event| {
        if event.id().0.starts_with("surf.") {
            let _ = handle.get_webview("chrome").and_then(|w| {
                w.emit_to("chrome", "menu://action", event.id().0.clone()).ok()
            });
        }
    });
}
