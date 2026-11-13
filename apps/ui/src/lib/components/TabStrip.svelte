<script lang="ts">
  import { tabs, openTab, closeTab, activateTab, setPinned } from "../tabs.svelte";
  import { ui, toggleTabSearch } from "../ui.svelte";
  import { tabTitle } from "../format";
  import Icon from "./Icon.svelte";
  import Favicon from "./Favicon.svelte";
</script>

<div class="strip" role="tablist" aria-label="分頁">
  <!-- The traffic lights own the first 78px; Chrome puts tab search here. -->
  <button class="tab-search" onclick={() => toggleTabSearch()} aria-label="搜尋分頁" title="搜尋分頁（⇧⌘A）">
    <Icon name="chevronDown" />
  </button>
  {#each tabs.list as tab (tab.id)}
    <button
      role="tab"
      class="tab"
      class:active={tab.id === tabs.activeId}
      class:pinned={tab.pinned}
      class:split={tabs.split ? tabs.split[0] === tab.id || tabs.split[1] === tab.id : false}
      aria-selected={tab.id === tabs.activeId}
      title={tabTitle(tab)}
      data-tab-id={tab.id}
      onclick={() => activateTab(tab.id)}
      oncontextmenu={(e) => {
        e.preventDefault();
        ui.tabMenu = { x: e.clientX, y: e.clientY, tabId: tab.id };
      }}
    >
      <span class="head">
        <Favicon url={tab.url} size={16} />
        <span class="label">{tabTitle(tab)}</span>
        {#if tab.incognito}
          <span class="private-mark" aria-label="無痕分頁" title="無痕">無痕</span>
        {/if}
        {#if tab.muted}
          <span class="muted" aria-label="分頁已靜音" title="已靜音">
            <Icon name="mute" />
          </span>
        {/if}
        {#if tab.loading}
          <span class="loading-dot" aria-label="載入中"></span>
        {/if}
      </span>
      {#if tab.id === tabs.activeId}
        <span class="underline" aria-hidden="true"></span>
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
    align-items: flex-start;
    gap: 6px;
    height: 40px;
    padding: 6px 6px 0 78px;
    background: var(--strip-surface);
    overflow-x: auto;
    scrollbar-width: none;
  }
  .strip::-webkit-scrollbar {
    display: none;
  }
  .tab-search {
    flex: none;
    display: grid;
    place-items: center;
    width: 28px;
    height: 24px;
    margin-top: 5px;
    border-radius: 6px;
    color: var(--text-secondary);
  }
  .tab-search:hover {
    background: var(--hover);
    color: var(--text);
  }
  .tab {
    position: relative;
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 48px;
    max-width: 240px;
    /* Chrome tabs share the strip equally: grow from zero, not from the
       content width, so tabs stay the same size however long their titles. */
    flex: 1 1 0;
    height: 34px;
    padding: 0 28px 0 10px;
    border-radius: 10px 10px 0 0;
    color: var(--text-secondary);
    font-size: 12px;
  }
  /* Pinned tabs keep their full size and title: the star toggle must
     not shrink the row (owner decision, overriding the icon-only form).
     Grouping left + accent star + hidden close still mark the state. */
  .tab.pinned {
    min-width: 48px;
  }
  .tab.split {
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .tab:hover {
    background: var(--hover);
  }
  /* The active tab is a continuation of the toolbar surface: same fill,
     and the GM3-style outward curves at its bottom corners connect it to
     the toolbar below. The accent underline stays as the Surf identity
     moment (DESIGN.md: accent only marks the active tab). */
  .tab.active {
    background: var(--toolbar-surface);
    color: var(--text);
    z-index: 2;
  }
  .tab.active::before,
  .tab.active::after {
    content: "";
    position: absolute;
    bottom: 0;
    width: 10px;
    height: 10px;
    pointer-events: none;
  }
  .tab.active::before {
    left: -10px;
    background: radial-gradient(circle at 0 0, transparent 9.5px, var(--toolbar-surface) 10px);
  }
  .tab.active::after {
    right: -10px;
    background: radial-gradient(circle at 100% 0, transparent 9.5px, var(--toolbar-surface) 10px);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    flex: 1;
  }
  /* The one deliberate accent moment: which tab you are in. */
  .underline {
    position: absolute;
    left: 8px;
    right: 8px;
    bottom: 0;
    height: 2px;
    background: var(--accent);
    pointer-events: none;
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
  .muted {
    flex: none;
    display: grid;
    place-items: center;
    color: var(--text-secondary);
  }
  .muted :global(svg) {
    width: 12px;
    height: 12px;
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
    flex: none;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    margin-top: 5px;
    border-radius: 6px;
    color: var(--text-secondary);
  }
  .new-tab:hover {
    background: var(--hover);
    color: var(--text);
  }
</style>
