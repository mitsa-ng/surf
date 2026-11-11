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

async function renderApp() {
  const target = document.createElement("div");
  document.body.appendChild(target);
  const app = mount(App, { target });
  // let onMount promises settle
  await new Promise((r) => setTimeout(r, 50));
  return { app, target };
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
});
