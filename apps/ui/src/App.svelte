<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { init, openTab, openIncognito, reopenClosed, closeTab, activateTab, tabs, navigateActive, historyAction } from "./lib/tabs.svelte";
  import { initUi, showBlockedNav, toggleBookmarksBar, toggleFind, togglePanel, toggleTabSearch, ui } from "./lib/ui.svelte";
  import { deeplinkTake, overlaySetOpen, panelSetOpen, tabZoom, type TabsSnapshot as IpcTabsSnapshot } from "./lib/ipc";
  import { runInternalCommand } from "./lib/internal";
  import { isMobileDevice } from "./lib/platform";
  import TabStrip from "./lib/components/TabStrip.svelte";
  import Toolbar from "./lib/components/Toolbar.svelte";
  import BookmarksBar from "./lib/components/BookmarksBar.svelte";
  import FindBar from "./lib/components/FindBar.svelte";
  import PasswordPrompt from "./lib/components/PasswordPrompt.svelte";
  import TabSearch from "./lib/components/TabSearch.svelte";
  import TabMenu from "./lib/components/TabMenu.svelte";
  import BottomBar from "./lib/components/BottomBar.svelte";
  import TabSheet from "./lib/components/TabSheet.svelte";
  import Panel from "./lib/components/Panel.svelte";
  import HelpDialog from "./lib/components/HelpDialog.svelte";

  const isMobile = isMobileDevice();
  let toolbar: Toolbar | undefined = $state();
  const activeUrl = $derived(tabs.list.find((t) => t.id === tabs.activeId)?.url ?? null);
  $effect(() => {
    if (isMobile) return;
    const open = ui.panel !== null;
    void panelSetOpen(open).catch((error) => console.error(String(error)));
  });

  // Any chrome overlay (tab menu, tab search, address suggestions, find
  // bar, help dialog, password prompt) needs the chrome webview to extend
  // below the chrome stack, above content. While the chrome covers more
  // than its stack, its body must be transparent so the page stays
  // visible underneath.
  $effect(() => {
    if (isMobile) return;
    const overlay =
      ui.tabMenu !== null ||
      ui.tabSearchOpen ||
      ui.suggestOpen ||
      ui.findOpen ||
      ui.help !== null ||
      ui.pwPrompt !== null;
    document.body.classList.toggle("chrome-expanded", ui.panel !== null || overlay);
    void overlaySetOpen(overlay).catch((error) => console.error(String(error)));
  });

  // Mobile: the tab sheet, panels, find bar, help dialog and the
  // omnibox suggestion dropdown render in the full-screen main webview,
  // which the native content webviews stack above. Opening any of them
  // must lift the main webview to the front (overlay_set_open ->
  // raise_main_chrome) or they stay hidden behind the active page and
  // cannot be tapped.
  $effect(() => {
    if (!isMobile) return;
    const overlay =
      tabs.sheetOpen ||
      ui.panel !== null ||
      ui.findOpen ||
      ui.help !== null ||
      ui.mobileMenu ||
      ui.omniOpen ||
      // Same overlay contract for the blocked-navigation toast.
      ui.blockedHost !== null;
    void overlaySetOpen(overlay).catch((error) => console.error(String(error)));
  });

  onMount(() => {
    void init();
    void initUi();
    if (isMobile) {
      // The Rust layout cannot derive mobile geometry itself: tauri
      // reports scale 1.0 on Android (so CSS px cannot be converted),
      // and tao keeps the pre-inset window size when system bars or the
      // IME shrink the decor frame. Report the bar and this viewport in
      // physical px instead; Rust re-lays-out content webviews on every
      // report, so keyboard and rotation flows stay live.
      const reportChrome = () => {
        const bar = document.querySelector<HTMLElement>(".bottom-bar");
        if (!bar) return;
        const dpr = window.devicePixelRatio;
        const bottomBarPx = Math.round(bar.offsetHeight * dpr);
        const viewportWPx = Math.round(window.innerWidth * dpr);
        const viewportHPx = Math.round(window.innerHeight * dpr);
        if (bottomBarPx > 0) {
          invoke("chrome_metrics_set", { bottomBarPx, viewportWPx, viewportHPx }).catch((e) =>
            console.error(String(e)),
          );
        }
      };
      setTimeout(() => void reportChrome(), 50);
      window.addEventListener("resize", () => void reportChrome());
      // Android back contract: MainActivity asks the chrome webview first;
      // closing one overlay consumes the press. Anything left is the page
      // history's turn, which Kotlin drives on the visible content webview.
      (window as unknown as { __surfHandleBack?: () => boolean }).__surfHandleBack = () => {
        if (tabs.sheetOpen) {
          tabs.sheetOpen = false;
        } else if (ui.mobileMenu) {
          ui.mobileMenu = false;
        } else if (ui.panel !== null) {
          ui.panel = null;
        } else if (ui.findOpen) {
          ui.findOpen = false;
        } else if (ui.help !== null) {
          ui.help = null;
        } else if (ui.omniOpen) {
          (document.activeElement as HTMLElement | null)?.blur();
        } else {
          return false;
        }
        return true;
      };
    }
    const onNavigate = (e: Event) => {
      const url = (e as CustomEvent<string>).detail;
      if (typeof url === "string") void navigateActive(url);
    };
    window.addEventListener("surf:navigate", onNavigate);
    const unlisten: Promise<UnlistenFn>[] = [
      listen<string>("menu://action", (e) => {
        switch (e.payload) {
          case "surf.new-tab":
            void openTab();
            break;
          case "surf.new-private":
            void openIncognito();
            break;
          case "surf.reopen-tab":
            void reopenClosed();
            break;
          case "surf.close-tab":
            if (tabs.activeId !== null) void closeTab(tabs.activeId);
            break;
          case "surf.reload":
            void historyAction("reload");
            break;
          case "surf.downloads":
            togglePanel("downloads");
            break;
          case "surf.tab-search":
            toggleTabSearch();
            break;
          case "surf.bookmarks-bar":
            toggleBookmarksBar();
            break;
          case "surf.zoom-in":
          case "surf.zoom-out":
          case "surf.zoom-reset":
            if (tabs.activeId !== null) {
              const mode = e.payload === "surf.zoom-in" ? "in" : e.payload === "surf.zoom-out" ? "out" : "reset";
              void tabZoom(tabs.activeId, mode).catch(console.error);
            }
            break;
          case "surf.bookmarks":
            togglePanel("bookmarks");
            break;
          case "surf.history":
            togglePanel("history");
            break;
          case "surf.extensions":
            togglePanel("extensions");
            break;
          case "surf.settings":
            togglePanel("settings");
            break;
          case "surf.find":
            toggleFind();
            break;
          case "surf.shortcuts":
            ui.help = "shortcuts";
            break;
          case "surf.about":
            ui.help = "about";
            break;
        }
      }),
      listen<string>("deeplink://open", (e) => runInternalCommand(e.payload)),
      // The blocked notice rides the tabs snapshot, one-shot Rust side.
      // Fired imperatively here: the same signal through a template {#if}
      // stayed invisible in the real macOS chrome webview, while this
      // DOM path rendered on both platforms.
      listen<IpcTabsSnapshot>("tabs://changed", (e) => {
        const blocked = e.payload.blocked;
        if (blocked && blocked.tab === tabs.activeId) showBlockedNav(blocked.url, blocked.seq);
      }),
    ];
    // A deep link that arrived before this UI mounted (CLI launch with a
    // URL) is queued Rust-side; take it once the tab model exists.
    void (async () => {
      try {
        await init();
        const queued = await deeplinkTake();
        if (queued) runInternalCommand(queued);
      } catch (e) {
        console.error(String(e));
      }
    })();
    return () => {
      window.removeEventListener("surf:navigate", onNavigate);
      void Promise.all(unlisten).then((fns) => fns.forEach((fn: UnlistenFn) => fn()));
    };
  });

  function onKeydown(e: KeyboardEvent) {
    // Esc closes the find bar from anywhere (Chrome parity); its own
    // input handler only sees Esc while focused, which left the bar
    // stranded open when focus had moved on.
    if (e.key === "Escape") {
      if (ui.findOpen) {
        e.preventDefault();
        ui.findOpen = false;
      }
      return;
    }
    const meta = e.metaKey || e.ctrlKey;
    if (!meta) return;
    if (e.key === "l") {
      e.preventDefault();
      toolbar?.focusAddressBar();
    } else if (e.key === "t") {
      e.preventDefault();
      void openTab();
    } else if (e.key === "w" && tabs.activeId !== null) {
      e.preventDefault();
      void closeTab(tabs.activeId);
    } else if (e.shiftKey && e.key.toLowerCase() === "b") {
      e.preventDefault();
      toggleBookmarksBar();
    } else if (e.shiftKey && e.key.toLowerCase() === "a") {
      e.preventDefault();
      toggleTabSearch();
    } else if (e.shiftKey && e.key.toLowerCase() === "j") {
      e.preventDefault();
      togglePanel("downloads");
    } else if (e.shiftKey && e.key.toLowerCase() === "n") {
      e.preventDefault();
      void openIncognito();
    } else if (e.shiftKey && e.key.toLowerCase() === "t") {
      e.preventDefault();
      void reopenClosed();
    } else if (e.key.toLowerCase() === "f") {
      e.preventDefault();
      toggleFind();
    } else if (/^[1-9]$/.test(e.key) && !e.shiftKey) {
      const target = tabs.list[Number(e.key) - 1];
      if (target) {
        e.preventDefault();
        void activateTab(target.id);
      }
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if isMobile}
  <main class="mobile">
    {#if tabs.sheetOpen}
      <TabSheet />
    {/if}
    <div class="content">
      {#if tabs.list.length === 0}
        <p>沒有分頁。點下方分頁按鈕新增。</p>
      {:else}
        <p class="mobile-note">多分頁已啟用；作用分頁：</p>
        <p class="mobile-url">{activeUrl ?? "（空白分頁）"}</p>
      {/if}
      {#if tabs.error}
        <p class="mobile-error">{tabs.error}</p>
      {/if}
    </div>
    <FindBar />
    <Panel />
    <HelpDialog />
    <BottomBar />
  </main>
{:else}
  <header style:height={`${87 + (ui.bookmarksBar ? 28 : 0)}px`}>
    <TabStrip />
    <Toolbar bind:this={toolbar} />
    {#if ui.bookmarksBar}
      <BookmarksBar />
    {/if}
    <FindBar />
    <PasswordPrompt />
    <TabSearch />
    <TabMenu />
  </header>
  <Panel />
  <HelpDialog />
  <!-- Content webviews are native children positioned below this chrome;
       nothing renders here. -->
{/if}
<style>
  header {
    position: fixed;
    inset: 0 0 auto 0;
    background: var(--panel);
  }
  .mobile {
    display: flex;
    flex-direction: column;
    /* dvh keeps the bottom bar above the home indicator on iOS, and
       tracks the window while the Android IME is open (the activity
       fits the decor, so adjustResize shrinks the viewport). */
    height: 100vh;
    height: 100dvh;
  }
  .content {
    flex: 1;
    display: grid;
    place-items: center;
    padding: 16px;
    background: var(--bg);
  }
  .content p {
    margin: 4px 0;
    color: var(--text-secondary);
    font-size: 14px;
    text-align: center;
    overflow-wrap: anywhere;
    max-width: 90vw;
  }
  .mobile-error {
    color: #b3261e;
    font-size: 12px;
    max-width: 90vw;
    overflow-wrap: anywhere;
  }
  .mobile-url {
    font-family: ui-monospace, monospace;
    font-size: 12px;
  }
</style>
