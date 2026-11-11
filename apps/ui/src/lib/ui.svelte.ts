import * as ipc from "./ipc";
import { tabs } from "./tabs.svelte";

export const ui = $state<{
  panel: null | "bookmarks" | "history" | "extensions" | "settings";
  extensions: ipc.ExtensionSummary[];
  bookmarks: ipc.BookmarkRow[];
  history: ipc.HistoryRow[];
  installError: string;
  installNotice: string;
  installPath: string;
  pendingInstall: null | { id: string; name: string; version: string; permissions: string[] };
  /** Desktop bookmarks bar visibility; mirrors the persisted setting. */
  bookmarksBar: boolean;
}>({
  panel: null,
  extensions: [],
  bookmarks: [],
  history: [],
  installError: "",
  installNotice: "",
  installPath: "",
  pendingInstall: null,
  bookmarksBar: false,
});

let started = false;

// The standard set Surf exposes to extensions; the consent dialog lists
// these before the extension may use any of them.
const GRANTABLE_APIS = ["storage", "tabs", "bookmarks", "alarms", "fetch"];

export async function initUi() {
  if (started) return;
  started = true;
  await Promise.all([
    refreshExtensions(),
    refreshBookmarks(),
    // The Rust side already applied the persisted flag at setup; this
    // only mirrors it in the UI so the header renders the right height.
    ipc
      .settingsGet("bookmarks_bar")
      .then((v) => (ui.bookmarksBar = v !== null && v !== ""))
      .catch(() => {}),
  ]);
}

/** Cmd/Ctrl+Shift+B: toggle the desktop bookmarks bar. */
export function toggleBookmarksBar() {
  ui.bookmarksBar = !ui.bookmarksBar;
  void ipc.bookmarksBarSetOpen(ui.bookmarksBar).catch((e) => console.error(String(e)));
}

export async function refreshExtensions() {
  ui.extensions = await ipc.extensionsList().catch(() => []);
}

export async function refreshBookmarks() {
  ui.bookmarks = await ipc.bookmarksList().catch(() => []);
}

export async function refreshHistory() {
  ui.history = await ipc.historyRecent(100).catch(() => []);
}

export function togglePanel(name: "bookmarks" | "history" | "extensions" | "settings") {
  if (ui.panel === name) {
    ui.panel = null;
    return;
  }
  ui.panel = name;
  if (name === "bookmarks") void refreshBookmarks();
  if (name === "history") void refreshHistory();
  if (name === "extensions") void refreshExtensions();
}

export async function proposeInstall() {
  ui.installError = "";
  ui.installNotice = "";
  const path = ui.installPath.trim();
  if (!path) return;
  try {
    const summary = await ipc.extensionInstall(path);
    // Registered but denied until the user grants; the dialog is the grant.
    ui.pendingInstall = { id: summary.id, name: summary.name, version: summary.version, permissions: [...GRANTABLE_APIS] };
  } catch (e) {
    ui.installError = String(e);
  }
}

export async function confirmInstall() {
  const pending = ui.pendingInstall;
  if (!pending) return;
  await ipc.extensionGrant(pending.id, pending.permissions).catch((e) => console.error(String(e)));
  ui.pendingInstall = null;
  ui.installNotice = `已安裝「${pending.name}」並授予權限`;
  await refreshExtensions();
}

export async function cancelInstall() {
  const pending = ui.pendingInstall;
  if (!pending) return;
  await ipc.extensionUninstall(pending.id).catch(() => {});
  ui.pendingInstall = null;
  await refreshExtensions();
}

export async function uninstall(id: string) {
  await ipc.extensionUninstall(id).catch((e) => console.error(String(e)));
  await refreshExtensions();
}

export async function openPopup(id: string) {
  await ipc.extPopupOpen(id).catch((e) => console.error(String(e)));
}

export async function openOptions(id: string) {
  await ipc.extOptionsOpen(id).catch((e) => console.error(String(e)));
}

export function isBookmarked(url: string | null): boolean {
  if (!url) return false;
  return ui.bookmarks.some((b) => b.url === url);
}

export async function toggleBookmark() {
  const active = tabs.list.find((t) => t.id === tabs.activeId);
  if (!active?.url) return;
  if (isBookmarked(active.url)) {
    ui.bookmarks = await ipc.bookmarksRemove(active.url);
  } else {
    ui.bookmarks = await ipc.bookmarksAdd(active.url, active.title ?? undefined);
  }
}
