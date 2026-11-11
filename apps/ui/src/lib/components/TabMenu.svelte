<script lang="ts">
  import { ui, refreshBookmarks } from "../ui.svelte";
  import { tabs, closeTab, openTab, setPinned } from "../tabs.svelte";
  import { splitClear, splitSet, tabOpen } from "../ipc";

  const menu = $derived(ui.tabMenu);
  const tab = $derived(menu ? tabs.list.find((t) => t.id === menu.tabId) ?? null : null);
  const isSplitMember = $derived(
    menu && tabs.split ? tabs.split[0] === menu.tabId || tabs.split[1] === menu.tabId : false,
  );

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

  async function duplicate() {
    if (tab?.url) await openTab(tab.url);
    close();
  }

  async function togglePin() {
    if (!tab) return;
    await setPinned(tab.id, !tab.pinned);
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
    <button role="menuitem" onclick={() => void copyUrl()} disabled={!tab.url}>複製網址</button>
    <button role="menuitem" onclick={() => void duplicate()} disabled={!tab.url}>複製分頁</button>
    <hr aria-hidden="true" />
    <button role="menuitem" onclick={() => void togglePin()}>{tab.pinned ? "取消釘選" : "釘選"}</button>
    {#if !isSplitMember}
      <button role="menuitem" onclick={() => void splitWith()}>並排顯示（分割畫面）</button>
    {:else}
      <button role="menuitem" onclick={() => void leaveSplit()}>結束分割畫面</button>
    {/if}
    <hr aria-hidden="true" />
    <button role="menuitem" class="danger" onclick={removeTab} disabled={tab.pinned}>關閉分頁</button>
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
    min-width: 200px;
    padding: 6px;
    background: var(--toolbar-surface);
    border-radius: 10px;
    box-shadow: 0 10px 34px rgba(0, 0, 0, 0.2);
  }
  button {
    height: 32px;
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
  hr {
    border: none;
    border-top: 1px solid var(--hairline);
    margin: 5px 6px;
  }
  .danger {
    color: #b3261e;
  }
  @media (prefers-color-scheme: dark) {
    .danger {
      /* Same hue, lightened for the dark surface (4.5:1+). */
      color: #f2b8b5;
    }
  }
</style>
