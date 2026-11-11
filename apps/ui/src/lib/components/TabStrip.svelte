<script lang="ts">
  import { tabs, openTab, closeTab, activateTab, setPinned } from "../tabs.svelte";
  import { ui } from "../ui.svelte";
  import { tabTitle } from "../format";
  import Icon from "./Icon.svelte";
</script>

<div class="strip" role="tablist" aria-label="分頁">
  {#each tabs.list as tab (tab.id)}
    <button
      role="tab"
      class="tab"
      class:active={tab.id === tabs.activeId}
      class:pinned={tab.pinned}
      class:split={tabs.split ? tabs.split[0] === tab.id || tabs.split[1] === tab.id : false}
      aria-selected={tab.id === tabs.activeId}
      title={tabTitle(tab)}
      onclick={() => activateTab(tab.id)}
      oncontextmenu={(e) => {
        e.preventDefault();
        ui.tabMenu = { x: e.clientX, y: e.clientY, tabId: tab.id };
      }}
    >
      <span class="label">{tabTitle(tab)}</span>
      {#if tab.incognito}
        <span class="private-mark" aria-label="無痕分頁" title="無痕">無痕</span>
      {/if}
      {#if tab.loading}
        <span class="loading-dot" aria-label="載入中"></span>
      {/if}
      <span
        class="pin"
        role="button"
        tabindex="-1"
        aria-label={tab.pinned ? "取消釘選" : "釘選"}
        title={tab.pinned ? "取消釘選" : "釘選"}
        onclick={(e) => {
          e.stopPropagation();
          void setPinned(tab.id, !tab.pinned);
        }}
        onkeydown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.stopPropagation();
            void setPinned(tab.id, !tab.pinned);
          }
        }}
      >
        {tab.pinned ? "★" : "☆"}
      </span>
      {#if !tab.pinned}
      <span
        class="close"
        role="button"
        tabindex="-1"
        aria-label={`關閉 ${tabTitle(tab)}`}
        onclick={(e) => {
          e.stopPropagation();
          closeTab(tab.id);
        }}
        onkeydown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.stopPropagation();
            closeTab(tab.id);
          }
        }}
      >
        <Icon name="close" />
      </span>
      {/if}
    </button>
  {/each}
  <button class="new-tab" onclick={() => openTab()} aria-label="新增分頁" title="新增分頁">
    <Icon name="plus" />
  </button>
</div>

<style>
  .strip {
    display: flex;
    align-items: stretch;
    gap: 1px;
    height: 36px;
    padding: 4px 6px 0;
    overflow-x: auto;
    scrollbar-width: none;
  }
  .strip::-webkit-scrollbar {
    display: none;
  }
  .tab {
    position: relative;
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    max-width: 200px;
    flex: 0 1 auto;
    padding: 0 28px 0 12px;
    border-radius: 6px 6px 0 0;
    color: var(--text-secondary);
  }
  .tab.pinned {
    max-width: 120px;
    padding: 0 24px 0 12px;
  }
  .tab.split {
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .tab:hover {
    background: var(--hover);
  }
  .tab.active {
    background: var(--bg);
    color: var(--text);
  }
  /* The one deliberate accent moment: which tab you are in. */
  .tab.active::after {
    content: "";
    position: absolute;
    left: 8px;
    right: 8px;
    bottom: 0;
    height: 2px;
    background: var(--accent);
  }
  .label {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .private-mark {
    flex: none;
    padding: 1px 5px;
    border: 1px solid var(--hairline);
    border-radius: 4px;
    color: var(--text-secondary);
    font-size: 10px;
    line-height: 1.2;
    user-select: none;
  }
  .loading-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    flex: none;
  }
  .pin {
    position: absolute;
    right: 20px;
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-radius: 4px;
    color: var(--text-secondary);
    font-size: 11px;
    line-height: 1;
    opacity: 0;
  }
  .tab.pinned .pin {
    right: 4px;
    color: var(--accent);
    opacity: 0.7;
  }
  .tab:hover .pin,
  .pin:focus-visible {
    opacity: 1;
  }
  .pin:hover {
    background: var(--hover);
    color: var(--text);
  }
  .close {
    position: absolute;
    right: 4px;
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 4px;
    color: var(--text-secondary);
    opacity: 0;
  }
  .close :global(svg) {
    width: 12px;
    height: 12px;
  }
  .tab:hover .close,
  .tab:focus-within .close,
  .close:focus-visible {
    opacity: 1;
  }
  .close:hover {
    background: var(--hover);
    color: var(--text);
  }
  .new-tab {
    display: grid;
    place-items: center;
    width: 30px;
    height: 28px;
    margin: 0 2px;
    border-radius: 6px;
    color: var(--text-secondary);
    align-self: center;
    flex: none;
  }
  .new-tab:hover {
    background: var(--hover);
    color: var(--text);
  }
</style>
