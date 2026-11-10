import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface TabState {
  id: number;
  url: string | null;
  title: string | null;
  loading: boolean;
}

export interface TabsSnapshot {
  tabs: TabState[];
  active: number | null;
}

export interface FilterCapabilities {
  block: boolean;
  redirect: boolean;
  modify_headers: boolean;
}

export const getTabs = () => invoke<TabsSnapshot>("get_tabs");

export const tabOpen = (url?: string) => invoke<TabsSnapshot>("tab_open", { url: url ?? null });

export const tabClose = (id: number) => invoke<TabsSnapshot>("tab_close", { id });

export const tabActivate = (id: number) => invoke<TabsSnapshot>("tab_activate", { id });

export const tabNavigate = (id: number, input: string) => invoke<TabsSnapshot>("tab_navigate", { id, input });

export const tabBack = (id: number) => invoke<void>("tab_back", { id });

export const tabForward = (id: number) => invoke<void>("tab_forward", { id });

export const tabReload = (id: number) => invoke<void>("tab_reload", { id });

export const filterCapabilities = () => invoke<FilterCapabilities>("filter_capabilities");

export const onTabsChanged = (handler: (snapshot: TabsSnapshot) => void): Promise<UnlistenFn> =>
  listen<TabsSnapshot>("tabs://changed", (event) => handler(event.payload));
