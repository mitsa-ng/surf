import * as ipc from "./ipc";

export const tabs = $state<{
  list: ipc.TabState[];
  activeId: number | null;
  sheetOpen: boolean;
}>({
  list: [],
  activeId: null,
  sheetOpen: false,
});

let started = false;

export function applySnapshot(snapshot: ipc.TabsSnapshot) {
  tabs.list = snapshot.tabs;
  tabs.activeId = snapshot.active;
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

export async function openTab() {
  applySnapshot(await ipc.tabOpen());
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
