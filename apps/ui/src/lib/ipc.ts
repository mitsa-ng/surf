import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface TabState {
  id: number;
  url: string | null;
  title: string | null;
  loading: boolean;
  incognito?: boolean;
  pinned?: boolean;
  /** Mirrored webview history state for the disabled nav buttons. */
  canBack?: boolean;
  canForward?: boolean;
}

export interface TabsSnapshot {
  tabs: TabState[];
  active: number | null;
  split?: [number, number] | null;
}

export interface FilterCapabilities {
  block: boolean;
  redirect: boolean;
  modify_headers: boolean;
}

export const getTabs = () => invoke<TabsSnapshot>("get_tabs");

export const tabOpen = (url?: string) => invoke<TabsSnapshot>("tab_open", { url: url ?? null });

export const tabOpenIncognito = () => invoke<TabsSnapshot>("tab_open_incognito");

export const tabSetPinned = (id: number, pinned: boolean) =>
  invoke<TabsSnapshot>("tab_set_pinned", { id, pinned });

export const tabReopenClosed = () => invoke<TabsSnapshot>("tab_reopen_closed");

export const splitSet = (left: number, right: number) =>
  invoke<void>("split_set", { left, right });

export const splitClear = () => invoke<void>("split_clear");

export const tabClose = (id: number) => invoke<TabsSnapshot>("tab_close", { id });

/** Close every other unpinned tab (Chrome's "close other tabs"). */
export const tabCloseOthers = (id: number) => invoke<TabsSnapshot>("tab_close_others", { id });

/** Close every unpinned tab to the right of `id` in strip order. */
export const tabCloseRight = (id: number) => invoke<TabsSnapshot>("tab_close_right", { id });

export const tabActivate = (id: number) => invoke<TabsSnapshot>("tab_activate", { id });

export const tabNavigate = (id: number, input: string) => invoke<TabsSnapshot>("tab_navigate", { id, input });

export const tabBack = (id: number) => invoke<void>("tab_back", { id });

export const tabForward = (id: number) => invoke<void>("tab_forward", { id });

export const tabReload = (id: number) => invoke<void>("tab_reload", { id });

export const tabStop = (id: number) => invoke<void>("tab_stop", { id });

export const filterCapabilities = () => invoke<FilterCapabilities>("filter_capabilities");

export interface ExtensionSummary {
  id: string;
  name: string;
  version: string;
  has_options: boolean;
}

export const extensionsList = () => invoke<ExtensionSummary[]>("extensions_list");

export const extensionInstall = (path: string) => invoke<ExtensionSummary>("extension_install", { path });

export const extensionUninstall = (id: string) => invoke<ExtensionSummary[]>("extension_uninstall", { id });

export const extensionGrant = (id: string, apis: string[]) => invoke<void>("extension_grant", { id, apis });

export const extPopupOpen = (id: string) => invoke<void>("ext_popup_open", { id });

export const extOptionsOpen = (id: string) => invoke<void>("ext_options_open", { id });

export interface BookmarkRow {
  url: string;
  title: string | null;
}

export const bookmarksList = () => invoke<BookmarkRow[]>("bookmarks_list");

export const bookmarksAdd = (url: string, title?: string) => invoke<BookmarkRow[]>("bookmarks_add", { url, title: title ?? null });

export const bookmarksRemove = (url: string) => invoke<BookmarkRow[]>("bookmarks_remove", { url });

export interface HistoryRow {
  url: string;
  title: string | null;
  visited_at: number;
}

export const historyRecent = (limit = 100) => invoke<HistoryRow[]>("history_recent", { limit });

export const rulesGet = () => invoke<string[]>("rules_get");

export const rulesSet = (filters: string[]) => invoke<string[]>("rules_set", { filters });

export const onTabsChanged = (handler: (snapshot: TabsSnapshot) => void): Promise<UnlistenFn> =>
  listen<TabsSnapshot>("tabs://changed", (event) => handler(event.payload));

export const syncExport = () => invoke<string>("sync_export");

export const syncImport = (path: string) => invoke<string>("sync_import", { path });

export const panelSetOpen = (open: boolean) => invoke<void>("panel_set_open", { open });

export const overlaySetOpen = (open: boolean) => invoke<void>("overlay_set_open", { open });

export const syncAutoGet = () => invoke<string | null>("sync_auto_get");

export const syncAutoSet = (path: string | null) => invoke<string | null>("sync_auto_set", { path });

export const pickSyncFile = () => invoke<string | null>("pick_sync_file");

export const historyClear = () => invoke<number>("history_clear");

export const bookmarksBarSetOpen = (open: boolean) => invoke<void>("bookmarks_bar_set_open", { open });

export const settingsGet = (key: string) => invoke<string | null>("settings_get", { key });

export type DownloadRow = { url: string; path: string; filename: string; success: boolean; at: number };

export const downloadsList = () => invoke<DownloadRow[]>("downloads_list");

export const downloadReveal = (path: string) => invoke<void>("download_reveal", { path });

export const findInPage = (id: number, query: string, backward: boolean) =>
  invoke<void>("find_in_page", { id, query, backward });

/** The content webview reports its live match count after each find. */
export const onFindCount = (handler: (tab: number, count: number) => void): Promise<UnlistenFn> =>
  listen<{ tab: number; count: number }>("find://count", (event) => handler(event.payload.tab, event.payload.count));

/** Take the cold-start deep link queued before this UI mounted. */
export const deeplinkTake = () => invoke<string | null>("deeplink_take");
