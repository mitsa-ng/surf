import * as ipc from "./ipc";
import { tabs } from "./tabs.svelte";

export const ui = $state<{
  panel: null | "bookmarks" | "history" | "extensions" | "settings" | "downloads" | "agent";
  extensions: ipc.ExtensionSummary[];
  bookmarks: ipc.BookmarkRow[];
  history: ipc.HistoryRow[];
  downloads: ipc.DownloadRow[];
  installError: string;
  installNotice: string;
  installPath: string;
  pendingInstall: null | { id: string; name: string; version: string; permissions: string[] };
  /** Desktop bookmarks bar visibility; mirrors the persisted setting. */
  bookmarksBar: boolean;
  /** Tab-search overlay (Cmd/Ctrl+Shift+A). */
  tabSearchOpen: boolean;
  /** Find-in-page bar (Cmd/Ctrl+F). */
  findOpen: boolean;
  /** Right-click menu on a tab strip row. */
  tabMenu: null | { x: number; y: number; tabId: number };
  /** Address suggestions visible (drives chrome expansion). */
  suggestOpen: boolean;
  /** Help dialog (menu 說明): keyboard shortcuts or about. */
  help: null | "shortcuts" | "about";
  /** Search engine setting ("duckduckgo" | "google" | "bing" | "custom"). */
  searchEngine: string;
  /** Custom engine template with {q}, when searchEngine is "custom". */
  searchEngineCustom: string;
  /** Live engine suggestions in the omnibox dropdown (setting-backed). */
  suggestLive: boolean;
  /** Login save prompt (host + user only; the password stays Rust-side). */
  pwPrompt: null | { host: string; user: string };
  /** Saved logins for the settings panel (host + user). */
  passwords: ipc.PasswordRow[];
  /** Mobile overflow menu (BottomBar 更多 button). */
  mobileMenu: boolean;
  /** Mobile: the omnibox suggestion dropdown is showing (the main
      webview must sit above the content views or the dropdown is
      untappable). */
  omniOpen: boolean;
}>({
  panel: null,
  extensions: [],
  bookmarks: [],
  history: [],
  downloads: [],
  installError: "",
  installNotice: "",
  installPath: "",
  pendingInstall: null,
  bookmarksBar: false,
  tabSearchOpen: false,
  findOpen: false,
  tabMenu: null,
  suggestOpen: false,
  help: null,
  searchEngine: "duckduckgo",
  searchEngineCustom: "",
  suggestLive: true,
  pwPrompt: null,
  passwords: [],
  mobileMenu: false,
  omniOpen: false,
});

const ENGINE_LABELS: Record<string, string> = {
  duckduckgo: "DuckDuckGo",
  google: "Google",
  bing: "Bing",
  custom: "搜尋",
};

export function searchEngineLabel(): string {
  return ENGINE_LABELS[ui.searchEngine] ?? "搜尋";
}

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
    refreshHistory(),
    refreshDownloads(),
    // The Rust side already applied the persisted flag at setup; this
    // only mirrors it in the UI so the header renders the right height.
    ipc
      .settingsGet("bookmarks_bar")
      .then((v) => (ui.bookmarksBar = v !== null && v !== ""))
      .catch(() => {}),
    ipc
      .settingsGet("search_engine")
      .then((v) => {
        if (v) ui.searchEngine = v;
      })
      .catch(() => {}),
    ipc
      .settingsGet("search_engine_custom")
      .then((v) => (ui.searchEngineCustom = v ?? ""))
      .catch(() => {}),
    ipc
      .settingsGet("suggest_live")
      .then((v) => (ui.suggestLive = v !== "0"))
      .catch(() => {}),
  ]);
  // Download events arrive from the download handlers; refresh the list.
  const { listen } = await import("@tauri-apps/api/event");
  for (const event of ["downloads://started", "downloads://finished"]) {
    void listen(event, () => void refreshDownloads()).catch(() => {});
  }
  // A submitted login asks whether to save (host + user only).
  void ipc
    .onPwPrompt((host, user) => {
      ui.pwPrompt = { host, user };
    })
    .catch(() => {});
}

export async function refreshPasswords() {
  ui.passwords = await ipc.passwordList().catch(() => []);
}

export async function savePromptedPassword() {
  ui.pwPrompt = null;
  await ipc.passwordSave().catch((e) => console.error(String(e)));
}

export async function dismissPromptedPassword() {
  ui.pwPrompt = null;
  await ipc.passwordDismiss().catch((e) => console.error(String(e)));
}

export async function deletePassword(host: string) {
  ui.passwords = await ipc.passwordDelete(host).catch(() => ui.passwords);
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

export async function refreshHistory(query = "") {
  const trimmed = query.trim();
  ui.history = await ipc.historySearch(trimmed).catch(() => []);
}

export async function refreshDownloads() {
  ui.downloads = await ipc.downloadsList().catch(() => []);
}

export function togglePanel(name: "bookmarks" | "history" | "extensions" | "settings" | "downloads" | "agent") {
  if (ui.panel === name) {
    ui.panel = null;
    return;
  }
  ui.panel = name;
  if (name === "bookmarks") void refreshBookmarks();
  if (name === "history") void refreshHistory();
  if (name === "extensions") void refreshExtensions();
  if (name === "downloads") void refreshDownloads();
}

/** Cmd/Ctrl+Shift+A: tab-search overlay. */
export function toggleTabSearch() {
  ui.tabSearchOpen = !ui.tabSearchOpen;
  if (ui.tabSearchOpen) ui.panel = null;
}

/** Cmd/Ctrl+F: find-in-page bar. */
export function toggleFind() {
  ui.findOpen = !ui.findOpen;
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
