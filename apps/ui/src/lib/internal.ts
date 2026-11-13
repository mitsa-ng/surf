import { tabs, openIncognito, reopenClosed, setPinned, navigateActive } from "./tabs.svelte";
import { togglePanel, toggleTabSearch } from "./ui.svelte";

// surf:// internal commands (Chrome's chrome:// pages). The same handler
// serves OS deep links and omnibox input, so there is exactly one action
// path per command.
//
// Hosts that take no argument map to a fixed action; surf://open?url=
// carries a target. Anything else under surf:// is unknown: the caller
// must not fall through to a webview load (nothing serves that scheme),
// so unknowns are reported instead of navigated.
const KNOWN = new Set(["settings", "private", "pin", "reopen", "split", "unsplit", "tabs", "open"]);

export function isInternalCommand(raw: string): boolean {
  return raw.trim().toLowerCase().startsWith("surf://");
}

/** Dispatch one internal command. Returns what happened. */
export function runInternalCommand(raw: string): "handled" | "unknown" | "ignored" {
  const url = raw.trim();
  const rest = url.slice("surf://".length);
  if (rest === "settings") {
    togglePanel("settings");
    return "handled";
  }
  if (rest === "private") {
    void openIncognito();
    return "handled";
  }
  if (rest === "pin") {
    const id = tabs.activeId;
    const tab = tabs.list.find((t) => t.id === id);
    if (id !== null && tab) {
      void setPinned(id, !tab.pinned);
      return "handled";
    }
    return "ignored";
  }
  if (rest === "reopen") {
    void reopenClosed();
    return "handled";
  }
  if (rest === "split") {
    const active = tabs.activeId;
    const other = tabs.list.find((t) => t.id !== active && !t.incognito)?.id ?? null;
    if (active !== null && other !== null) {
      void import("./ipc").then((ipc) => ipc.splitSet(active, other)).catch(console.error);
      return "handled";
    }
    return "ignored";
  }
  if (rest === "unsplit") {
    void import("./ipc").then((ipc) => ipc.splitClear()).catch(console.error);
    return "handled";
  }
  if (rest === "tabs") {
    toggleTabSearch();
    return "handled";
  }
  if (rest.startsWith("open?url=")) {
    const target = decodeURIComponent(rest.slice("open?url=".length));
    if (target) {
      void navigateActive(target);
      return "handled";
    }
    return "ignored";
  }
  return "unknown";
}
