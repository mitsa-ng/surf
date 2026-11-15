<script lang="ts">
  import { ui, refreshBookmarks } from "../ui.svelte";
  import { tabs, closeTab, openTab, setPinned, applySnapshot } from "../tabs.svelte";
  import { isInternalCommand, runInternalCommand } from "../internal";
  import { splitClear, splitSet, tabNavigate, tabReload, tabCloseOthers, tabCloseRight, tabSetMuted } from "../ipc";

  const menu = $derived(ui.tabMenu);
  const tab = $derived(menu ? tabs.list.find((t) => t.id === menu.tabId) ?? null : null);
  const isSplitMember = $derived(
    menu && tabs.split ? tabs.split[0] === menu.tabId || tabs.split[1] === menu.tabId : false,
  );
  // Chrome leaves pinned tabs alive in both batch closes.
  const hasOtherTabs = $derived(
    menu ? tabs.list.some((t) => t.id !== menu.tabId && !t.pinned) : false,
  );
  const hasRightTabs = $derived.by(() => {
    if (!menu) return false;
    const pos = tabs.list.findIndex((t) => t.id === menu.tabId);
    return pos >= 0 && tabs.list.slice(pos + 1).some((t) => !t.pinned);
  });

  let menuEl: HTMLDivElement | undefined = $state();

  function close() {
    // Focus returns to the tab the menu came from, so keyboard users are
    // not dropped into a void after closing.
    const from = menu?.tabId;
    ui.tabMenu = null;
    if (from !== undefined) {
      (document.querySelector(`[data-tab-id="${from}"]`) as HTMLElement | null)?.focus();
    }
  }

  function focusables(): HTMLButtonElement[] {
    return Array.from(menuEl?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? []);
  }

  // Chrome menus are keyboard-first: focus enters the menu on open and
  // arrows walk the items (R-32).
  $effect(() => {
    if (menu) {
      setTimeout(() => focusables()[0]?.focus(), 0);
    }
  });

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      close();
      return;
    }
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const items = focusables();
    if (!items.length) return;
    const current = items.indexOf(document.activeElement as HTMLButtonElement);
    const next =
      e.key === "ArrowDown"
        ? items[(current + 1 + items.length) % items.length]
        : items[(current - 1 + items.length) % items.length];
    next!.focus();
  }

  function newTab() {
    void openTab();
    close();
  }

  async function reload() {
    if (!tab) return;
    await tabReload(tab.id).catch((e) => console.error(String(e)));
    close();
  }

  async function duplicate() {
    if (tab?.url) await openTab(tab.url);
    close();
  }

  // Chrome's paste-and-go: the clipboard text resolves as URL or search
  // in this tab. The click gesture satisfies clipboard read.
  async function pasteAndGo() {
    const target = tab;
    close();
    if (!target) return;
    try {
      const text = (await navigator.clipboard.readText()).trim();
      if (!text) return;
      // Same guard as the omnibox: surf:// text is a browser command,
      // and nothing serves that scheme to a webview.
      if (isInternalCommand(text)) {
        runInternalCommand(text);
        return;
      }
      applySnapshot(await tabNavigate(target.id, text));
    } catch (e) {
      console.error(String(e));
    }
  }

  async function copyUrl() {
    if (!tab?.url) return;
    try {
      await navigator.clipboard.writeText(tab.url);
    } catch {
      // Clipboard needs a user gesture on some webviews; the menu click is
      // one, so failures here are rare. Nothing else to do honestly.
    }
    close();
  }

  async function togglePin() {
    if (!tab) return;
    await setPinned(tab.id, !tab.pinned);
    close();
  }

  async function toggleMute() {
    if (!tab) return;
    await tabSetMuted(tab.id, !tab.muted).then(applySnapshot).catch((e) => console.error(String(e)));
    close();
  }

  async function splitWith() {
    if (!menu || !tab) return;
    const other = tabs.activeId !== tab.id ? tabs.activeId : tabs.list.find((t) => t.id !== tab.id)?.id ?? null;
    if (other === null) return;
    await splitSet(other, tab.id).catch((e) => console.error(String(e)));
    close();
  }

  async function leaveSplit() {
    await splitClear().catch((e) => console.error(String(e)));
    close();
  }

  function removeTab() {
    if (!tab) return;
    void closeTab(tab.id);
    close();
  }

  async function closeOthers() {
    if (!menu) return;
    close();
    await tabCloseOthers(menu.tabId).then(applySnapshot).catch((e) => console.error(String(e)));
  }

  async function closeRight() {
    if (!menu) return;
    close();
    await tabCloseRight(menu.tabId).then(applySnapshot).catch((e) => console.error(String(e)));
  }

  // Refresh the star state after pin toggles from the menu.
  $effect(() => {
    void refreshBookmarks();
  });
</script>

{#if menu && tab}
  <div class="scrim" role="presentation" onclick={close} oncontextmenu={(e) => { e.preventDefault(); close(); }}></div>
  <div
    class="menu"
    role="menu"
    aria-label="分頁操作"
    tabindex="-1"
    bind:this={menuEl}
    onkeydown={onKeydown}
    style:left={`${menu.x}px`}
    style:top={`${menu.y}px`}
  >
    <button role="menuitem" onclick={newTab}>
      <span>開新分頁</span><span class="sc">⌘T</span>
    </button>
    <button role="menuitem" onclick={() => void reload()}>
      <span>重新載入</span><span class="sc">⌘R</span>
    </button>
    <button role="menuitem" onclick={() => void duplicate()} disabled={!tab.url}>複製</button>
    <button role="menuitem" onclick={() => void pasteAndGo()}>貼上並前往</button>
    <button role="menuitem" onclick={() => void copyUrl()} disabled={!tab.url}>複製網址</button>
    <hr aria-hidden="true" />
    <button role="menuitem" onclick={() => void togglePin()}>{tab.pinned ? "取消釘選" : "釘選"}</button>
    {#if !isSplitMember}
      <button role="menuitem" onclick={() => void splitWith()}>並排顯示（分割畫面）</button>
    {:else}
      <button role="menuitem" onclick={() => void leaveSplit()}>結束分割畫面</button>
    {/if}
    <button role="menuitem" onclick={() => void toggleMute()}>{tab.muted ? "取消靜音分頁" : "靜音分頁"}</button>
    <hr aria-hidden="true" />
    <button role="menuitem" onclick={removeTab}>
      <span>關閉分頁</span><span class="sc">⌘W</span>
    </button>
    <button role="menuitem" onclick={() => void closeOthers()} disabled={!hasOtherTabs}>關閉其他分頁</button>
    <button role="menuitem" onclick={() => void closeRight()} disabled={!hasRightTabs}>關閉右側分頁</button>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 7;
  }
  .menu {
    position: fixed;
    z-index: 8;
    display: flex;
    flex-direction: column;
    min-width: 220px;
    padding: 6px;
    background: var(--toolbar-surface);
    border-radius: 10px;
    box-shadow: 0 10px 34px rgba(0, 0, 0, 0.2);
  }
  button {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    height: 30px;
    padding: 0 12px;
    border-radius: 8px;
    text-align: left;
    font-size: 13px;
    color: var(--text);
  }
  button:hover:not(:disabled),
  button:focus-visible {
    background: var(--hover);
  }
  button:disabled {
    opacity: 0.4;
  }
  .sc {
    color: var(--text-secondary);
    font-size: 12px;
  }
  hr {
    border: none;
    border-top: 1px solid var(--hairline);
    margin: 5px 6px;
  }
</style>
