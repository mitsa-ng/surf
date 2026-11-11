<script lang="ts">
  import { ui, refreshBookmarks } from "../ui.svelte";
  import { tabs, closeTab, openTab, setPinned } from "../tabs.svelte";
  import { splitClear, splitSet, tabOpen } from "../ipc";

  const menu = $derived(ui.tabMenu);
  const tab = $derived(menu ? tabs.list.find((t) => t.id === menu.tabId) ?? null : null);
  const isSplitMember = $derived(
    menu && tabs.split ? tabs.split[0] === menu.tabId || tabs.split[1] === menu.tabId : false,
  );

  function close() {
    ui.tabMenu = null;
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
    style:left={`${menu.x}px`}
    style:top={`${menu.y}px`}
  >
    <button role="menuitem" onclick={() => void copyUrl()} disabled={!tab.url}>複製網址</button>
    <button role="menuitem" onclick={() => void duplicate()} disabled={!tab.url}>複製分頁</button>
    <button role="menuitem" onclick={() => void togglePin()}>{tab.pinned ? "取消釘選" : "釘選"}</button>
    {#if !isSplitMember}
      <button role="menuitem" onclick={() => void splitWith()}>並排顯示（分割畫面）</button>
    {:else}
      <button role="menuitem" onclick={() => void leaveSplit()}>結束分割畫面</button>
    {/if}
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
    padding: 4px;
    background: var(--panel);
    border: 1px solid var(--hairline);
    border-radius: 8px;
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.14);
  }
  button {
    height: 32px;
    padding: 0 12px;
    border-radius: 6px;
    text-align: left;
    font-size: 13px;
    color: var(--text);
  }
  button:hover:not(:disabled) {
    background: var(--hover);
  }
  button:disabled {
    opacity: 0.4;
  }
  .danger {
    color: #b3261e;
  }
</style>
