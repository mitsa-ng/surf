// @vitest-environment jsdom
// Mounts the real App with mocked Tauri IPC and asserts the chrome
// actually renders its toolbar — the regression this guards against is a
// blank toolbar row in the shipped webview.
import { mount, unmount } from "svelte";
import { describe, expect, it, vi, beforeEach } from "vitest";

const invokeMock = vi.fn(async (cmd: string, _args?: object) => {
  switch (cmd) {
    case "get_tabs":
      return { tabs: [], active: null };
    case "bookmarks_list":
    case "history_recent":
    case "extensions_list":
    case "downloads_list":
      return [];
    default:
      return null;
  }
});
const listenMock = vi.fn(async (_event?: string, _handler?: unknown) => () => {});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args?: object) => invokeMock(cmd, args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: (event: string, handler: (event: unknown) => void) => listenMock(event, handler),
}));

import App from "../App.svelte";
import Favicon from "../lib/components/Favicon.svelte";
import TabMenu from "../lib/components/TabMenu.svelte";
import { applySnapshot } from "../lib/tabs.svelte";
import { ui } from "../lib/ui.svelte";
import type { TabsSnapshot } from "../lib/ipc";

async function renderApp() {
  const target = document.createElement("div");
  document.body.appendChild(target);
  const app = mount(App, { target });
  // let onMount promises settle
  await new Promise((r) => setTimeout(r, 50));
  return { app, target };
}

function seedTabs(tabs: Partial<TabsSnapshot["tabs"][number]>[]) {
  applySnapshot({ tabs: tabs as TabsSnapshot["tabs"], active: (tabs[0]!.id ?? null) as number | null, split: null });
}

describe("chrome rendering", () => {
  beforeEach(() => {
    invokeMock.mockClear();
    listenMock.mockClear();
    document.body.innerHTML = "";
  });

  it("renders the toolbar row with nav buttons and the address input", async () => {
    const { app, target } = await renderApp();
    const toolbar = target.querySelector(".toolbar");
    expect(toolbar, "toolbar element missing").toBeTruthy();
    const navs = toolbar!.querySelectorAll("button.nav");
    expect(navs.length, "expected back/forward/reload buttons").toBe(3);
    const input = toolbar!.querySelector<HTMLInputElement>("input.address");
    expect(input, "address input missing").toBeTruthy();
    expect(input!.placeholder).toBe("搜尋或輸入網址");
    await unmount(app);
  });

  it("renders the tab strip new-tab button", async () => {
    const { app, target } = await renderApp();
    expect(target.querySelector(".new-tab"), "new tab button missing").toBeTruthy();
    await unmount(app);
  });

  it("renders page action buttons next to the address bar", async () => {
    const { app, target } = await renderApp();
    const actions = target.querySelector(".actions");
    expect(actions, "page actions row missing").toBeTruthy();
    expect(actions!.querySelectorAll("button").length).toBeGreaterThanOrEqual(4);
    await unmount(app);
  });

  it("disables back and forward from the mirrored history flags", async () => {
    seedTabs([{ id: 1, url: "https://a.example/", title: "A", loading: false, canBack: true, canForward: false }]);
    const { app, target } = await renderApp();
    const navs = target.querySelectorAll<HTMLButtonElement>(".toolbar button.nav");
    expect(navs.length).toBe(3);
    // The snapshot order is back, forward, reload; the flags decide the
    // disabled state, never the store's guess.
    const [back, forward] = [navs[0]!, navs[1]!];
    expect(back.disabled, "back should be enabled with canBack").toBe(false);
    expect(forward.disabled, "forward should be disabled without canForward").toBe(true);
    await unmount(app);
    seedTabs([{ id: 1, url: "https://a.example/", title: "A", loading: false, canBack: false, canForward: true }]);
    const second = await renderApp();
    const navs2 = second.target.querySelectorAll<HTMLButtonElement>(".toolbar button.nav");
    expect(navs2[0]!.disabled, "back should be disabled without canBack").toBe(true);
    expect(navs2[1]!.disabled, "forward should be enabled with canForward").toBe(false);
    await unmount(second.app);
  });

  it("keeps the bookmark star inside the omnibox", async () => {
    seedTabs([{ id: 1, url: "https://a.example/", title: "A", loading: false }]);
    const { app, target } = await renderApp();
    const wrap = target.querySelector(".address-wrap");
    expect(wrap?.querySelector("button.star"), "star missing from the omnibox").toBeTruthy();
    await unmount(app);
  });
});

describe("favicon placeholder", () => {
  it("is deterministic per domain and shows the first letter", async () => {
    const target = document.createElement("div");
    document.body.appendChild(target);
    const a = mount(Favicon, { target, props: { url: "https://example.org/x", size: 16 } });
    await new Promise((r) => setTimeout(r, 10));
    const first = target.querySelector<HTMLElement>(".fav")!;
    const hue = first.style.getPropertyValue("--h");
    const letter = first.textContent;
    await unmount(a);

    const target2 = document.createElement("div");
    document.body.appendChild(target2);
    const b = mount(Favicon, { target: target2, props: { url: "https://example.org/y", size: 16 } });
    await new Promise((r) => setTimeout(r, 10));
    const again = target2.querySelector<HTMLElement>(".fav")!;
    expect(again.style.getPropertyValue("--h"), "same domain keeps the same hue").toBe(hue);
    expect(again.textContent, "same domain keeps the same letter").toBe(letter);
    await unmount(b);

    const target3 = document.createElement("div");
    document.body.appendChild(target3);
    const c = mount(Favicon, { target: target3, props: { url: "https://wikipedia.org/", size: 16 } });
    await new Promise((r) => setTimeout(r, 10));
    const other = target3.querySelector<HTMLElement>(".fav")!;
    expect(other.style.getPropertyValue("--h")).not.toBe(hue);
    expect(other.textContent, "letter comes from the hostname").toBe("W");
    await unmount(c);
  });
});

describe("tab menu", () => {
  it("opens for a real tab and moves focus with the arrow keys", async () => {
    seedTabs([{ id: 1, url: "https://a.example/", title: "A", loading: false }]);
    const target = document.createElement("div");
    document.body.appendChild(target);
    ui.tabMenu = { x: 20, y: 20, tabId: 1 };
    const app = mount(TabMenu, { target });
    await new Promise((r) => setTimeout(r, 10));
    const menu = target.querySelector<HTMLDivElement>(".menu");
    expect(menu, "menu missing").toBeTruthy();
    const items = menu!.querySelectorAll<HTMLButtonElement>("button:not(:disabled)");
    expect(items.length).toBeGreaterThan(1);
    expect(document.activeElement, "first item focused on open").toBe(items[0]);
    menu!.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
    expect(document.activeElement).toBe(items[1]);
    menu!.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    await new Promise((r) => setTimeout(r, 10));
    expect(ui.tabMenu, "escape closes the menu").toBe(null);
    await unmount(app);
    document.body.innerHTML = "";
  });
});
