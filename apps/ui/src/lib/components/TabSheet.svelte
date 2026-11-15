<script lang="ts">
  import { tabs, openTab, closeTab, activateTab, setPinned, applySnapshot } from "../tabs.svelte";
  import { tabTitle } from "../format";
  import { splitClear, splitSet, tabCloseOthers, tabCloseRight, tabNavigate, tabReload, tabSetMuted } from "../ipc";
  import { isInternalCommand, runInternalCommand } from "../internal";
  import Icon from "./Icon.svelte";

  let query = $state("");
  let actionFor = $state<number | null>(null);
  // Viewport rect of the row whose menu is open; the menu is
  // position:fixed (absolute children cannot escape the sheet's
  // scroll clip) and opens upward from here.
  let menuAnchorTop = $state(0);

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return tabs.list;
    return tabs.list.filter(
      (t) => (t.title ?? "").toLowerCase().includes(q) || (t.url ?? "").toLowerCase().includes(q),
    );
  });

  function menuAction(run: () => void) {
    actionFor = null;
    run();
  }

  function inSplit(id: number) {
    return tabs.split !== null && (tabs.split[0] === id || tabs.split[1] === id);
  }
  function hasOthers(id: number) {
    return tabs.list.some((t) => t.id !== id && !t.pinned);
  }
  function hasRight(id: number) {
    const idx = tabs.list.findIndex((t) => t.id === id);
    return idx >= 0 && tabs.list.slice(idx + 1).some((t) => !t.pinned);
  }

  async function duplicate(id: number) {
    const tab = tabs.list.find((t) => t.id === id);
    if (tab?.url) await openTab(tab.url);
  }

  async function pasteAndGo(id: number) {
    try {
      const text = (await navigator.clipboard.readText()).trim();
      if (!text) return;
      if (isInternalCommand(text)) {
        runInternalCommand(text);
        return;
      }
      applySnapshot(await tabNavigate(id, text));
    } catch (e) {
      console.error(String(e));
    }
  }

  async function copyUrl(id: number) {
    const tab = tabs.list.find((t) => t.id === id);
    if (!tab?.url) return;
    try {
      await navigator.clipboard.writeText(tab.url);
    } catch {
      // Clipboard needs a user gesture; the menu tap is one.
    }
  }

  function splitWith(id: number) {
    const other = tabs.activeId !== id ? tabs.activeId : tabs.list.find((t) => t.id !== id)?.id ?? null;
    if (other !== null) void splitSet(id, other).catch(console.error);
  }

  function toggleMute(id: number, muted: boolean) {
    void tabSetMuted(id, muted).then(applySnapshot).catch((e) => console.error(String(e)));
  }
  function closeOthers(id: number) {
    void tabCloseOthers(id).then(applySnapshot).catch((e) => console.error(String(e)));
  }
  function closeRight(id: number) {
    void tabCloseRight(id).then(applySnapshot).catch((e) => console.error(String(e)));
  }
</script>

<div
  class="scrim"
  role="presentation"
  onclick={() => (tabs.sheetOpen = false)}
  oncontextmenu={(e) => {
    e.preventDefault();
    tabs.sheetOpen = false;
  }}
></div>
<div class="sheet" role="dialog" aria-modal="true" aria-label="分頁">
  {#if tabs.list.length > 3}
    <input
      class="filter"
      bind:value={query}
      type="search"
      placeholder="搜尋分頁"
      aria-label="搜尋分頁"
      spellcheck="false"
    />
  {/if}
  <ul>
    {#each filtered as tab (tab.id)}
      <li>
        {#if tab.pinned}
          <span class="pin-mark" aria-label="已釘選"><Icon name="star" /></span>
        {/if}
        {#if tab.muted}
          <span class="mute-mark" aria-label="已靜音"><Icon name="mute" /></span>
        {/if}
        <button
          class="row"
          class:active={tab.id === tabs.activeId}
          aria-current={tab.id === tabs.activeId ? "page" : undefined}
          onclick={() => activateTab(tab.id)}
        >
          <span class="label">{tabTitle(tab)}</span>
          {#if tab.loading}
            <span class="loading-dot" aria-label="載入中"></span>
          {/if}
        </button>
        <button
          class="more"
          aria-label={`${tabTitle(tab)} 操作`}
          aria-expanded={actionFor === tab.id}
          onclick={(e) => {
            const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
            if (actionFor === tab.id) {
              actionFor = null;
            } else {
              menuAnchorTop = rect.top;
              actionFor = tab.id;
            }
          }}
        >
          <Icon name="list" />
        </button>
        <button class="close" aria-label={`關閉 ${tabTitle(tab)}`} onclick={() => closeTab(tab.id)}>
          <Icon name="close" />
        </button>
        {#if actionFor === tab.id}
          <div
            class="actions"
            role="menu"
            aria-label="分頁操作"
            style:bottom={`${Math.max(8, Math.round(window.innerHeight - menuAnchorTop + 8))}px`}
          >
            <button role="menuitem" onclick={() => menuAction(() => void setPinned(tab.id, !tab.pinned))}>
              {tab.pinned ? "取消釘選" : "釘選"}
            </button>
            <button role="menuitem" onclick={() => menuAction(() => void tabReload(tab.id).catch(console.error))}>重新載入</button>
            {#if tab.url}
              <button role="menuitem" onclick={() => menuAction(() => void duplicate(tab.id))}>複製分頁</button>
              <button role="menuitem" onclick={() => menuAction(() => void copyUrl(tab.id))}>複製網址</button>
            {/if}
            <button role="menuitem" onclick={() => menuAction(() => void pasteAndGo(tab.id))}>貼上並前往</button>
            <button role="menuitem" onclick={() => menuAction(() => toggleMute(tab.id, !tab.muted))}>
              {tab.muted ? "取消靜音" : "靜音"}
            </button>
            {#if !inSplit(tab.id)}
              <button role="menuitem" onclick={() => menuAction(() => splitWith(tab.id))}>並排顯示</button>
            {:else}
              <button role="menuitem" onclick={() => menuAction(() => void splitClear().catch(console.error))}>結束並排</button>
            {/if}
            <button role="menuitem" disabled={!hasOthers(tab.id)} onclick={() => menuAction(() => closeOthers(tab.id))}>
              關閉其他分頁
            </button>
            <button role="menuitem" disabled={!hasRight(tab.id)} onclick={() => menuAction(() => closeRight(tab.id))}>
              關閉右側分頁
            </button>
          </div>
        {/if}
      </li>
    {:else}
      <li class="empty">沒有符合的分頁。</li>
    {/each}
  </ul>
  <button class="new" onclick={() => openTab()}>
    <Icon name="plus" />
    新增分頁
  </button>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: rgba(0, 0, 0, 0.32);
  }
  .sheet {
    position: fixed;
    z-index: 41;
    left: 0;
    right: 0;
    bottom: 0;
    max-height: 70vh;
    overflow-y: auto;
    padding: 8px 8px calc(28px + env(safe-area-inset-bottom));
    border-radius: 12px 12px 0 0;
    background: var(--panel);
  }
  .filter {
    width: 100%;
    height: 34px;
    margin-bottom: 6px;
    padding: 0 10px;
    border: 1px solid var(--hairline);
    border-radius: 8px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
    font-size: 13px;
  }
  .filter:focus {
    outline: none;
    border-color: var(--accent);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    position: relative;
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .pin-mark,
  .mute-mark {
    flex: none;
    display: grid;
    place-items: center;
    width: 18px;
    color: var(--accent);
  }
  .mute-mark {
    color: var(--text-secondary);
  }
  .pin-mark :global(svg),
  .mute-mark :global(svg) {
    width: 11px;
    height: 11px;
  }
  .row {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 44px;
    padding: 0 10px;
    border-radius: 8px;
    text-align: left;
    color: var(--text);
  }
  .row.active {
    background: var(--hover);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .row:active {
    background: var(--hover);
  }
  .label {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .loading-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    flex: none;
  }
  .more,
  .close {
    display: grid;
    place-items: center;
    width: 36px;
    height: 44px;
    border-radius: 8px;
    color: var(--text-secondary);
    flex: none;
  }
  .more :global(svg) {
    width: 13px;
    height: 13px;
  }
  .close :global(svg) {
    width: 14px;
    height: 14px;
  }
  .more:active,
  .close:active {
    background: var(--hover);
    color: var(--text);
  }
  .actions {
    /* Fixed, not absolute: an absolutely-positioned child cannot
       escape the sheet's scroll clip, and the nine-item menu is taller
       than the sheet's lower rows have below them. Anchored inline
       (bottom = above the opening row) and self-scrolling. */
    position: fixed;
    right: 4px;
    z-index: 42;
    max-height: 70vh;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    min-width: 168px;
    padding: 4px;
    border-radius: 10px;
    background: var(--toolbar-surface);
    box-shadow: 0 10px 34px rgba(0, 0, 0, 0.24);
  }
  .actions button {
    min-height: 40px;
    padding: 0 12px;
    border-radius: 7px;
    text-align: left;
    font-size: 13px;
    color: var(--text);
  }
  .actions button:disabled {
    opacity: 0.4;
  }
  .actions button:not(:disabled):active {
    background: var(--hover);
  }
  .empty {
    padding: 12px 0;
    color: var(--text-secondary);
    font-size: 13px;
  }
  .new {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    width: 100%;
    height: 44px;
    margin-top: 4px;
    border-radius: 8px;
    color: var(--text);
  }
  .new:active {
    background: var(--hover);
  }
</style>
