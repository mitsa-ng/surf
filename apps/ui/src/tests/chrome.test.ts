// @vitest-environment jsdom
// Mounts the real App with mocked Tauri IPC and asserts the chrome
// actually renders its toolbar — the regression this guards against is a
// blank toolbar row in the shipped webview.
import { mount, unmount } from "svelte";
import { describe, expect, it, vi, beforeEach } from "vitest";

const invokeMockImpl = async (cmd: string, _args?: object) => {
  switch (cmd) {
    case "get_tabs":
      return { tabs: [], active: null };
    case "bookmarks_list":
    case "history_recent":
    case "history_search":
    case "extensions_list":
    case "downloads_list":
    case "agent_chat_history":
      return [];
    default:
      return null;
  }
};
const invokeMock = vi.fn(invokeMockImpl);
const listenMock = vi.fn(async (_event?: string, _handler?: unknown) => () => {});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args?: object) => invokeMock(cmd, args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: (event: string, handler: (event: unknown) => void) => listenMock(event, handler),
}));

import App from "../App.svelte";
import BottomBar from "../lib/components/BottomBar.svelte";
import Toolbar from "../lib/components/Toolbar.svelte";
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

  it("shows the full chrome-parity item set with shortcut labels", async () => {
    seedTabs([
      { id: 1, url: "https://a.example/", title: "A", loading: false },
      { id: 2, url: "https://b.example/", title: "B", loading: false },
    ]);
    const target = document.createElement("div");
    document.body.appendChild(target);
    ui.tabMenu = { x: 20, y: 20, tabId: 1 };
    const app = mount(TabMenu, { target });
    await new Promise((r) => setTimeout(r, 10));
    const labels = Array.from(target.querySelectorAll<HTMLButtonElement>(".menu button")).map(
      (b) => b.textContent ?? "",
    );
    for (const wanted of ["開新分頁", "重新載入", "複製", "貼上並前往", "複製網址", "釘選", "關閉分頁", "關閉其他分頁", "關閉右側分頁"]) {
      expect(labels.some((l) => l.includes(wanted)), `menu item missing: ${wanted}`).toBe(true);
    }
    expect(labels.some((l) => l.includes("⌘T")), "⌘T shortcut label").toBe(true);
    expect(labels.some((l) => l.includes("⌘R")), "⌘R shortcut label").toBe(true);
    expect(labels.some((l) => l.includes("⌘W")), "⌘W shortcut label").toBe(true);
    // With another unpinned tab present, both batch closes are enabled.
    const byText = (frag: string) =>
      Array.from(target.querySelectorAll<HTMLButtonElement>(".menu button")).find((b) =>
        (b.textContent ?? "").includes(frag),
      )!;
    expect(byText("關閉其他分頁").disabled).toBe(false);
    expect(byText("關閉右側分頁").disabled).toBe(false);
    await unmount(app);

    // On the last tab both batch closes gray out, like Chrome.
    seedTabs([{ id: 9, url: "https://solo.example/", title: "S", loading: false }]);
    ui.tabMenu = { x: 20, y: 20, tabId: 9 };
    const target2 = document.createElement("div");
    document.body.appendChild(target2);
    const app2 = mount(TabMenu, { target: target2 });
    await new Promise((r) => setTimeout(r, 10));
    const byText2 = (frag: string) =>
      Array.from(target2.querySelectorAll<HTMLButtonElement>(".menu button")).find((b) =>
        (b.textContent ?? "").includes(frag),
      )!;
    expect(byText2("關閉其他分頁").disabled, "solo tab grays close-others").toBe(true);
    expect(byText2("關閉右側分頁").disabled, "last tab grays close-right").toBe(true);
    await unmount(app2);
    document.body.innerHTML = "";
  });

  it("paste-and-go runs surf:// text as a browser command, never a page load", async () => {
    seedTabs([{ id: 5, url: "https://a.example/", title: "A", loading: false }]);
    const calls: string[] = [];
    invokeMock.mockImplementation((async (cmd: string) => {
      calls.push(cmd);
      if (cmd === "get_tabs") return { tabs: [], active: null };
      // TabMenu refreshes bookmarks on mount; a null here would poison
      // ui.bookmarks for every later test.
      if (cmd === "bookmarks_list") return [];
      return null;
    }) as typeof invokeMockImpl);
    // jsdom ships no clipboard; the stub feeds paste-and-go directly.
    vi.stubGlobal("navigator", { ...navigator, clipboard: { readText: async () => "surf://settings" } });
    const target = document.createElement("div");
    document.body.appendChild(target);
    ui.tabMenu = { x: 20, y: 20, tabId: 5 };
    const app = mount(TabMenu, { target });
    await new Promise((r) => setTimeout(r, 10));
    const paste = Array.from(target.querySelectorAll<HTMLButtonElement>(".menu button")).find((b) =>
      (b.textContent ?? "").includes("貼上並前往"),
    )!;
    paste.click();
    await new Promise((r) => setTimeout(r, 30));
    expect(ui.panel, "internal command ran").toBe("settings");
    expect(calls).not.toContain("tab_navigate");
    vi.unstubAllGlobals();
    invokeMock.mockImplementation(invokeMockImpl);
    await unmount(app);
    document.body.innerHTML = "";
  });
});

describe("interaction fluency fixes", () => {
  beforeEach(() => {
    invokeMock.mockImplementation(invokeMockImpl);
    document.body.innerHTML = "";
    ui.panel = null;
    ui.findOpen = false;
  });

  it("Escape closes the find bar even when its input is not focused", async () => {
    const { app } = await renderApp();
    ui.findOpen = true;
    await new Promise((r) => setTimeout(r, 10));
    // Focus is on the body: the find input's own Esc handler cannot
    // see this keypress; the window-level handler must.
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    await new Promise((r) => setTimeout(r, 10));
    expect(ui.findOpen, "global Esc closed the find bar").toBe(false);
    await unmount(app);
    document.body.innerHTML = "";
  });

  it("mobile omnibox selects its text on focus so typing replaces it", async () => {
    seedTabs([{ id: 3, url: "https://example.org/x", title: "X", loading: false }]);
    const target = document.createElement("div");
    document.body.appendChild(target);
    const bar = mount(BottomBar, { target });
    await new Promise((r) => setTimeout(r, 20));
    const input = target.querySelector<HTMLInputElement>("input.address")!;
    // The model refill ran unfocused; focusing must select the shown
    // URL (Chrome parity, same as the desktop toolbar's onfocus).
    input.focus();
    await new Promise((r) => setTimeout(r, 30));
    expect(input.selectionStart).toBe(0);
    expect(input.selectionEnd).toBe(input.value.length);
    expect(input.selectionEnd, "the whole address is selected").toBeGreaterThan(0);
    await unmount(bar);
    document.body.innerHTML = "";
  });

  it("desktop omnibox keeps select-all across the click gesture", async () => {
    seedTabs([{ id: 7, url: "https://example.org/y", title: "Y", loading: false }]);
    const target = document.createElement("div");
    document.body.appendChild(target);
    const tl = mount(Toolbar, { target });
    await new Promise((r) => setTimeout(r, 20));
    const input = target.querySelector<HTMLInputElement>("input.address")!;
    // Chrome parity: one cold click selects all even though the
    // mouseup caret placement lands after the focus-time select().
    input.focus();
    input.dispatchEvent(new MouseEvent("mouseup", { bubbles: true }));
    await new Promise((r) => setTimeout(r, 30));
    expect(input.selectionStart).toBe(0);
    expect(input.selectionEnd, "the whole address is selected").toBe(input.value.length);
    await unmount(tl);
    document.body.innerHTML = "";
  });

  it("names a blocked navigation in a toast, active tab only", async () => {
    const { app } = await renderApp();
    // Seed after mount: init() applies the (empty) boot snapshot over
    // anything seeded earlier. The block rides the tabs://changed
    // snapshot (one-shot Rust side); a background tab's block must not
    // surface.
    seedTabs([{ id: 5, url: "https://start.example/", title: "S", loading: false }]);
    const handler = listenMock.mock.calls.find(([event]) => event === "tabs://changed")![1] as (
      e: { payload: TabsSnapshot },
    ) => void;
    const snap = (blocked: TabsSnapshot["blocked"]): TabsSnapshot => ({
      tabs: [{ id: 5, url: "https://start.example/", title: "S", loading: false }],
      active: 5,
      split: null,
      blocked,
    });
    handler({ payload: snap({ tab: 9, url: "https://blocked.example/", seq: 0 }) });
    await new Promise((r) => setTimeout(r, 10));
    expect(document.querySelector(".blocked-toast"), "another tab's block stays quiet").toBeNull();
    handler({ payload: snap({ tab: 5, url: "https://blocked.example/page", seq: 1 }) });
    await new Promise((r) => setTimeout(r, 10));
    expect(document.querySelector(".blocked-toast")?.textContent).toContain("blocked.example");
    await unmount(app);
    document.body.innerHTML = "";
  });

  it("Android back closes one overlay at a time, then yields to page history", async () => {
    // jsdom keeps userAgent on the prototype: save the value, not the
    // descriptor, or the stub leaks into every later test in this file.
    const prevUA = navigator.userAgent;
    Object.defineProperty(navigator, "userAgent", { value: "Android 16", configurable: true });
    try {
      const { app } = await renderApp();
      const back = (window as unknown as { __surfHandleBack?: () => boolean }).__surfHandleBack;
      expect(back, "App exposes the Android back hook").toBeTypeOf("function");
      // Nothing open: the visible content webview's history is next
      // (Kotlin drives WebView.goBack; the hook only owns overlays).
      expect(back!()).toBe(false);
      ui.panel = "settings";
      expect(back!(), "panel open is consumed by the hook").toBe(true);
      expect(ui.panel).toBeNull();
      expect(back!()).toBe(false);
      await unmount(app);
    } finally {
      Object.defineProperty(navigator, "userAgent", { value: prevUA, configurable: true });
    }
    document.body.innerHTML = "";
  });
});

describe("agent panel", () => {
  beforeEach(() => {
    invokeMock.mockImplementation(invokeMockImpl);
    listenMock.mockClear();
    document.body.innerHTML = "";
    ui.panel = null;
  });

  it("renders the chat panel with a greeting and a composer", async () => {
    const { app, target } = await renderApp();
    const actions = target.querySelector(".actions")!;
    const agentBtn = Array.from(actions.querySelectorAll<HTMLButtonElement>("button")).find((b) =>
      (b.getAttribute("aria-label") ?? "").includes("Agent"),
    );
    expect(agentBtn, "agent toolbar button missing").toBeTruthy();
    agentBtn!.click();
    await new Promise((r) => setTimeout(r, 80));
    const drawer = target.querySelector(".drawer");
    expect(drawer, "panel drawer missing").toBeTruthy();
    expect(drawer!.querySelector(".agent-panel"), "chat panel missing").toBeTruthy();
    expect(drawer!.querySelector(".greet"), "greeting missing in an empty thread").toBeTruthy();
    const input = drawer!.querySelector<HTMLInputElement>("input[aria-label='傳訊息給 agent']");
    expect(input, "composer input missing").toBeTruthy();
    await unmount(app);
  });

  it("shows the local demo source label when nothing is configured", async () => {
    const { app, target } = await renderApp();
    const actions = target.querySelector(".actions")!;
    const agentBtn = Array.from(actions.querySelectorAll<HTMLButtonElement>("button")).find((b) =>
      (b.getAttribute("aria-label") ?? "").includes("Agent"),
    )!;
    agentBtn.click();
    await new Promise((r) => setTimeout(r, 120));
    const drawer = target.querySelector(".drawer")!;
    const label = drawer.querySelector(".source");
    // Both config commands return null under the default mock; the
    // source precedence falls through to the local demo.
    expect(label?.textContent, "demo source label missing").toContain("本地示範模式");
    await unmount(app);
  });

  it("locks the send button while a run is active and releases it after", async () => {
    const { app, target } = await renderApp();
    const actions = target.querySelector(".actions")!;
    const agentBtn = Array.from(actions.querySelectorAll<HTMLButtonElement>("button")).find((b) =>
      (b.getAttribute("aria-label") ?? "").includes("Agent"),
    )!;
    agentBtn.click();
    await new Promise((r) => setTimeout(r, 120));
    const drawer = target.querySelector(".drawer")!;
    const input = drawer.querySelector<HTMLInputElement>("input[aria-label='傳訊息給 agent']")!;
    // The panel registered its agui://event listener; drive the same
    // wire the Rust run loop emits on.
    const registration = listenMock.mock.calls.find(([ev]) => ev === "agui://event");
    expect(registration, "agui://event listener missing").toBeTruthy();
    const handler = registration![1] as (event: { payload: unknown }) => void;

    input.value = "hello";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await new Promise((r) => setTimeout(r, 10));
    const send = drawer.querySelector<HTMLButtonElement>(".composer button[type='submit']")!;
    expect(send.disabled, "send enabled with text and no run").toBe(false);

    handler({ payload: { type: "RUN_STARTED" } });
    await new Promise((r) => setTimeout(r, 10));
    expect(send.disabled, "run-active locks send even with text").toBe(true);

    handler({ payload: { type: "RUN_ERROR", message: "x" } });
    await new Promise((r) => setTimeout(r, 10));
    expect(send.disabled, "run end releases send").toBe(false);
    await unmount(app);
  });

  it("sends the typed text through agent_chat_send on Enter", async () => {
    const calls: Array<[string, object?]> = [];
    invokeMock.mockImplementation((async (cmd: string, args?: object) => {
      calls.push([cmd, args]);
      if (cmd === "get_tabs") return { tabs: [], active: null };
      // A null bookmarks_list poisons ui.bookmarks for every later
      // test (the paste-and-go comment warns about exactly this).
      if (cmd === "bookmarks_list") return [];
      if (cmd === "agent_chat_history") return [];
      return null;
    }) as typeof invokeMockImpl);
    const { app, target } = await renderApp();
    const actions = target.querySelector(".actions")!;
    const agentBtn = Array.from(actions.querySelectorAll<HTMLButtonElement>("button")).find((b) =>
      (b.getAttribute("aria-label") ?? "").includes("Agent"),
    )!;
    agentBtn.click();
    await new Promise((r) => setTimeout(r, 80));
    const drawer = target.querySelector(".drawer")!;
    const input = drawer.querySelector<HTMLInputElement>("input[aria-label='傳訊息給 agent']")!;
    input.value = "現在幾個分頁？";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await new Promise((r) => setTimeout(r, 10));
    input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    await new Promise((r) => setTimeout(r, 20));
    const sent = calls.find(([cmd]) => cmd === "agent_chat_send");
    expect(sent, "agent_chat_send not invoked").toBeTruthy();
    expect(sent![1], "send payload carries the typed text").toEqual({ text: "現在幾個分頁？" });
    invokeMock.mockImplementation(invokeMockImpl);
    await unmount(app);
    document.body.innerHTML = "";
  });
});

describe("settings app icon", () => {
  beforeEach(() => {
    invokeMock.mockImplementation(invokeMockImpl);
    document.body.innerHTML = "";
    ui.panel = null;
  });

  it("shows the four icon options with auto selected by default", async () => {
    const { app, target } = await renderApp();
    ui.panel = "settings";
    await new Promise((r) => setTimeout(r, 60));
    const buttons = target.querySelectorAll<HTMLButtonElement>(".icon-options button");
    expect(buttons.length, "four icon options").toBe(4);
    expect(buttons[0]!.classList.contains("selected"), "auto is the default").toBe(true);
    expect(buttons[0]!.textContent).toContain("自動");
    await unmount(app);
  });

  it("saves the picked variant through app_icon_set", async () => {
    const calls: Array<[string, object?]> = [];
    invokeMock.mockImplementation((async (cmd: string, args?: object) => {
      calls.push([cmd, args]);
      if (cmd === "get_tabs") return { tabs: [], active: null };
      if (cmd === "bookmarks_list" || cmd === "history_recent") return [];
      if (cmd === "app_icon_get") return { choice: "auto" };
      return null;
    }) as typeof invokeMockImpl);
    const { app, target } = await renderApp();
    ui.panel = "settings";
    await new Promise((r) => setTimeout(r, 60));
    const blue = Array.from(target.querySelectorAll<HTMLButtonElement>(".icon-options button")).find((b) =>
      b.textContent!.includes("海藍"),
    )!;
    blue.click();
    await new Promise((r) => setTimeout(r, 20));
    const sent = calls.find(([cmd]) => cmd === "app_icon_set");
    expect(sent, "app_icon_set not invoked").toBeTruthy();
    expect(sent![1]).toEqual({ choice: "blue" });
    invokeMock.mockImplementation(invokeMockImpl);
    await unmount(app);
    document.body.innerHTML = "";
  });
});

describe("internal surf:// commands", () => {
  beforeEach(() => {
    // mockImplementation overrides persist across mockClear; restore the
    // default so later describes keep the sane list-returning mock.
    invokeMock.mockImplementation(invokeMockImpl);
    ui.panel = null;
    ui.tabSearchOpen = false;
  });

  it("runs a known command and never reaches the webview load path", async () => {
    const { runInternalCommand, isInternalCommand } = await import("../lib/internal");
    expect(isInternalCommand("surf://settings")).toBe(true);
    expect(isInternalCommand("https://surf.io")).toBe(false);
    const calls: string[] = [];
    invokeMock.mockImplementation(async (cmd: string) => {
      calls.push(cmd);
      if (cmd === "get_tabs") return { tabs: [], active: null };
      if (cmd === "split_set") return null;
      return null;
    });
    expect(runInternalCommand("surf://settings")).toBe("handled");
    expect(ui.panel).toBe("settings");
    expect(runInternalCommand("surf://tabs")).toBe("handled");
    expect(ui.tabSearchOpen).toBe(true);
    expect(runInternalCommand("surf://unsplit")).toBe("handled");
    await new Promise((r) => setTimeout(r, 20));
    expect(calls).toContain("split_clear");
    // No tab_navigate ever fired: internal commands are not page loads.
    expect(calls).not.toContain("tab_navigate");
  });

  it("reports unknown hosts instead of stranding the tab", async () => {
    const { runInternalCommand } = await import("../lib/internal");
    expect(runInternalCommand("surf://nonexistent")).toBe("unknown");
  });

  it("navigates through the omnibox router: internal commands never load", async () => {
    const { navigateActive } = await import("../lib/tabs.svelte");
    seedTabs([{ id: 5, url: "https://a.example/", title: "A", loading: false }]);
    const calls: string[] = [];
    invokeMock.mockImplementation(async (cmd: string) => {
      calls.push(cmd);
      if (cmd === "get_tabs") return { tabs: [], active: null };
      return null;
    });
    await navigateActive("surf://settings");
    expect(ui.panel).toBe("settings");
    expect(calls).not.toContain("tab_navigate");
    ui.panel = null;
    await navigateActive("https://example.org/");
    expect(calls).toContain("tab_navigate");
  });
});

describe("help dialog", () => {
  beforeEach(() => {
    // The omnibox-router test above swaps the implementation; restore it.
    invokeMock.mockImplementation(invokeMockImpl);
    document.body.innerHTML = "";
    ui.help = null;
  });

  it("opens the shortcuts table and closes on Escape", async () => {
    const { app, target } = await renderApp();
    ui.help = "shortcuts";
    await new Promise((r) => setTimeout(r, 10));
    const dialog = target.querySelector('[role="dialog"]');
    expect(dialog, "dialog missing").toBeTruthy();
    expect(dialog!.querySelectorAll("tr").length).toBeGreaterThan(6);
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    await new Promise((r) => setTimeout(r, 10));
    expect(ui.help).toBe(null);
    await unmount(app);
  });

  it("shows the about sheet with a real version from the app api", async () => {
    const { app, target } = await renderApp();
    ui.help = "about";
    await new Promise((r) => setTimeout(r, 30));
    const dialog = target.querySelector('[role="dialog"]');
    expect(dialog, "dialog missing").toBeTruthy();
    expect(dialog!.textContent).toContain("安靜極簡");
    await unmount(app);
  });
});

describe("omnibox live suggestions", () => {
  beforeEach(() => {
    invokeMock.mockImplementation(invokeMockImpl);
    document.body.innerHTML = "";
  });

  it("renders engine rows above the local rows once they resolve", async () => {
    invokeMock.mockImplementation((async (cmd: string) => {
      if (cmd === "get_tabs") return { tabs: [], active: null };
      if (cmd === "suggest") return ["weather 台南", "weather forecast"];
      return [];
    }) as typeof invokeMockImpl);
    const { app, target } = await renderApp();
    const input = target.querySelector<HTMLInputElement>("input.address");
    expect(input, "address input missing").toBeTruthy();
    input!.focus();
    input!.value = "weath";
    input!.dispatchEvent(new Event("input", { bubbles: true }));
    // debounce (150ms) + microtasks
    await new Promise((r) => setTimeout(r, 300));
    const rows = target.querySelectorAll('[role="option"]');
    const text = Array.from(rows).map((r) => r.textContent ?? "");
    expect(text.some((t) => t.includes("weather 台南")), "engine row missing").toBe(true);
    expect(text.some((t) => t.includes("weather forecast")), "engine row missing").toBe(true);
    expect(text.some((t) => t.includes("weath")), "local search row missing").toBe(true);
    await unmount(app);
  });

  it("selects a row on pointerdown before the touch blur can tear the dropdown down", async () => {
    const seen: Array<{ cmd: string; args: Record<string, unknown> }> = [];
    invokeMock.mockImplementation((async (cmd: string, args: Record<string, unknown>) => {
      if (cmd === "get_tabs") return { tabs: [{ id: 1, url: "https://example.org/" }], active: 1 };
      if (cmd === "suggest") return [];
      seen.push({ cmd, args });
      return null;
    }) as typeof invokeMockImpl);
    seedTabs([{ id: 1, url: "https://example.org/" }]);
    const target = document.createElement("div");
    document.body.appendChild(target);
    const bar = mount(BottomBar, { target });
    const input = target.querySelector<HTMLInputElement>("input.address");
    input!.focus();
    input!.value = "weath";
    input!.dispatchEvent(new Event("input", { bubbles: true }));
    await new Promise((r) => setTimeout(r, 250));
    const row = target.querySelector<HTMLButtonElement>('[role="option"]');
    expect(row, "suggestion row missing").toBeTruthy();
    // Android moves focus on touchstart: pointerdown is the only event
    // still guaranteed to land while the row is attached.
    row!.dispatchEvent(new Event("pointerdown", { bubbles: true, cancelable: true }));
    await new Promise((r) => setTimeout(r, 100));
    const nav = seen.find((s) => s.cmd === "tab_navigate");
    expect(nav, "pointerdown did not navigate").toBeTruthy();
    expect(nav!.args.input).toBe("weath");
    await unmount(bar);
  });
});
