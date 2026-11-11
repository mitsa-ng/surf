import * as ipc from "./ipc";

export const tabs = $state<{
  list: ipc.TabState[];
  activeId: number | null;
  split: [number, number] | null;
  sheetOpen: boolean;
  error: string;
}>({
  list: [],
  activeId: null,
  split: null,
  sheetOpen: false,
  error: "",
});

let started = false;

export function applySnapshot(snapshot: ipc.TabsSnapshot) {
  tabs.list = snapshot.tabs;
  tabs.activeId = snapshot.active;
  tabs.split = snapshot.split ?? null;
}

export function activeTab(): ipc.TabState | null {
  return tabs.list.find((t) => t.id === tabs.activeId) ?? null;
}

export async function init() {
  if (started) return;
  started = true;
  applySnapshot(await ipc.getTabs());
  await ipc.onTabsChanged(applySnapshot);
}

export async function openTab(url?: string) {
  try {
    applySnapshot(await ipc.tabOpen(url));
    tabs.error = "";
  } catch (e) {
    tabs.error = String(e);
  }
}

export async function setPinned(id: number, pinned: boolean) {
  try {
    applySnapshot(await ipc.tabSetPinned(id, pinned));
    tabs.error = "";
  } catch (e) {
    tabs.error = String(e);
  }
}

export async function reopenClosed() {
  try {
    applySnapshot(await ipc.tabReopenClosed());
    tabs.error = "";
  } catch (e) {
    tabs.error = String(e);
  }
}

export async function openIncognito() {
  try {
    applySnapshot(await ipc.tabOpenIncognito());
    tabs.error = "";
  } catch (e) {
    tabs.error = String(e);
  }
}

export async function closeTab(id: number) {
  try {
    applySnapshot(await ipc.tabClose(id));
  } catch (e) {
    console.error(String(e));
  }
}

export async function activateTab(id: number) {
  applySnapshot(await ipc.tabActivate(id));
  tabs.sheetOpen = false;
}

export async function navigateActive(input: string) {
  const id = tabs.activeId;
  if (id === null) return;
  try {
    applySnapshot(await ipc.tabNavigate(id, input));
  } catch (e) {
    console.error(String(e));
  }
}

export async function historyAction(action: "back" | "forward" | "reload") {
  const id = tabs.activeId;
  if (id === null) return;
  const call = { back: ipc.tabBack, forward: ipc.tabForward, reload: ipc.tabReload }[action];
  try {
    await call(id);
  } catch (e) {
    console.error(String(e));
  }
}
