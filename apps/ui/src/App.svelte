<script lang="ts">
  import { onMount } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { init, openTab, openIncognito, reopenClosed, closeTab, activateTab, tabs, navigateActive, historyAction } from "./lib/tabs.svelte";
  import { initUi, toggleBookmarksBar, toggleFind, togglePanel, toggleTabSearch, ui } from "./lib/ui.svelte";
  import { deeplinkTake, overlaySetOpen, panelSetOpen, tabZoom } from "./lib/ipc";
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
      ui.tabMenu !== null || ui.tabSearchOpen || ui.suggestOpen || ui.findOpen || ui.help !== null || ui.pwPrompt !== null;
    document.body.classList.toggle("chrome-expanded", ui.panel !== null || overlay);
    void overlaySetOpen(overlay).catch((error) => console.error(String(error)));
  });

  onMount(() => {
    void init();
    void initUi();
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
        <p class="mobile-note">分頁內容以每分頁視窗呈現；作用分頁：</p>
        <p class="mobile-url">{activeUrl ?? "（空白分頁）"}</p>
      {/if}
      {#if tabs.error}
        <p class="mobile-error">{tabs.error}</p>
      {/if}
    </div>
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
    /* dvh keeps the bottom bar above the home indicator on iOS. */
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
