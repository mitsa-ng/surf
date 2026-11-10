<script lang="ts">
  import { tabs, openTab, closeTab, activateTab } from "../tabs.svelte";
  import { tabTitle } from "../format";
  import Icon from "./Icon.svelte";
</script>

<div
  class="scrim"
  role="presentation"
  onclick={() => (tabs.sheetOpen = false)}
></div>
<div class="sheet" role="dialog" aria-modal="true" aria-label="分頁">
  <ul>
    {#each tabs.list as tab (tab.id)}
      <li>
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
        <button class="close" aria-label={`關閉 ${tabTitle(tab)}`} onclick={() => closeTab(tab.id)}>
          <Icon name="close" />
        </button>
      </li>
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
    background: rgba(0, 0, 0, 0.32);
  }
  .sheet {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    max-height: 70vh;
    overflow-y: auto;
    padding: 8px 8px calc(28px + env(safe-area-inset-bottom));
    border-radius: 12px 12px 0 0;
    background: var(--panel);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .row {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 44px;
    padding: 0 12px;
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
  .close {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: 8px;
    color: var(--text-secondary);
    flex: none;
  }
  .close :global(svg) {
    width: 14px;
    height: 14px;
  }
  .close:active {
    background: var(--hover);
    color: var(--text);
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
