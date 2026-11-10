import type { TabState } from "./ipc";

const MAX_ADDRESS_CHARS = 96;

// What the address bar shows for a URL: scheme and trailing slash off,
// query kept, over-long paths truncated in the middle.
export function displayAddress(url: string | null): string {
  if (!url) return "";
  let rest = url.replace(/^[a-z][a-z0-9+.-]*:\/\//i, "");
  if (rest.endsWith("/") && !rest.slice(0, -1).includes("/")) {
    rest = rest.slice(0, -1);
  }
  if (rest.length <= MAX_ADDRESS_CHARS) return rest;
  const head = rest.slice(0, MAX_ADDRESS_CHARS - 24);
  const tail = rest.slice(-20);
  return `${head}…${tail}`;
}

export function hostOf(url: string | null): string | null {
  if (!url) return null;
  try {
    return new URL(url).host || null;
  } catch {
    return null;
  }
}

// Tab label before the page reports its own title.
export function tabTitle(tab: TabState): string {
  if (tab.title) return tab.title;
  const host = hostOf(tab.url);
  return host ?? "新分頁";
}
