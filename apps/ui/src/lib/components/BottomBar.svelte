<script lang="ts">
  import { tabs, navigateActive, historyAction } from "../tabs.svelte";
  import { displayAddress } from "../format";
  import Icon from "./Icon.svelte";

  let input = $state("");
  let focused = $state(false);

  $effect(() => {
    const shown = displayAddress(activeTab()?.url ?? null);
    if (!focused) input = shown;
  });

  function activeTab() {
    return tabs.list.find((t) => t.id === tabs.activeId) ?? null;
  }

  function submit() {
    if (input.trim()) navigateActive(input);
    (document.activeElement as HTMLElement)?.blur();
  }
</script>

<nav class="bottom-bar">
  <button
    class="tabs-button"
    onclick={() => (tabs.sheetOpen = !tabs.sheetOpen)}
    aria-label="分頁列表"
    aria-expanded={tabs.sheetOpen}
  >
    <Icon name="tabs" />
    <span class="count">{tabs.list.length}</span>
  </button>
  <input
    class="address"
    bind:value={input}
    type="text"
    enterkeyhint="go"
    spellcheck="false"
    autocomplete="off"
    placeholder="搜尋或輸入網址"
    aria-label="網址與搜尋"
    onfocus={() => (focused = true)}
    onblur={() => (focused = false)}
    onkeydown={(e) => {
      if (e.key === "Enter") submit();
    }}
  />
  <button class="nav" onclick={() => historyAction("reload")} aria-label="重新載入">
    <Icon name="reload" />
  </button>
</nav>

<style>
  /* Mobile chrome sits at the bottom with thumb-reachable 44px targets. */
  .bottom-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px max(8px, env(safe-area-inset-left)) calc(8px + env(safe-area-inset-bottom))
      max(8px, env(safe-area-inset-right));
    border-top: 1px solid var(--hairline);
    background: var(--panel);
  }
  .tabs-button,
  .nav {
    position: relative;
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: 8px;
    color: var(--text);
    flex: none;
  }
  .tabs-button:active,
  .nav:active {
    background: var(--hover);
  }
  .count {
    position: absolute;
    bottom: 4px;
    right: 4px;
    min-width: 14px;
    height: 14px;
    padding: 0 3px;
    border-radius: 7px;
    background: var(--accent);
    color: #fff;
    font-size: 10px;
    line-height: 14px;
    text-align: center;
  }
  .address {
    flex: 1;
    min-width: 0;
    height: 44px;
    padding: 0 12px;
    border: 1px solid transparent;
    border-radius: 8px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
    font-size: 14px;
    user-select: text;
  }
  .address:focus {
    outline: none;
    border-color: var(--accent);
  }
</style>
