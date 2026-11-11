<script lang="ts">
  import { tabs, navigateActive, historyAction } from "../tabs.svelte";
  import { displayAddress } from "../format";
  import Icon from "./Icon.svelte";
  import PageActions from "./PageActions.svelte";

  let input = $state("");
  let focused = $state(false);
  let inputEl: HTMLInputElement | undefined = $state();
  const privateMode = $derived(activeTab()?.incognito ?? false);

  // Refill from the model whenever the address changes and we are not editing.
  $effect(() => {
    const shown = displayAddress(activeTab()?.url ?? null);
    if (!focused) input = shown;
  });

  function activeTab() {
    return tabs.list.find((t) => t.id === tabs.activeId) ?? null;
  }

  export function focusAddressBar() {
    inputEl?.focus();
    inputEl?.select();
  }

  function submit() {
    if (input.trim()) navigateActive(input);
    inputEl?.blur();
  }

  function reset() {
    input = displayAddress(activeTab()?.url ?? null);
    inputEl?.blur();
  }
</script>

<div class="toolbar">
  <button class="nav" onclick={() => historyAction("back")} aria-label="上一頁" title="上一頁">
    <Icon name="back" />
  </button>
  <button class="nav" onclick={() => historyAction("forward")} aria-label="下一頁" title="下一頁">
    <Icon name="forward" />
  </button>
  <button class="nav" onclick={() => historyAction("reload")} aria-label="重新載入" title="重新載入">
    <Icon name="reload" />
  </button>
  <input
    bind:this={inputEl}
    bind:value={input}
    class="address"
    class:loading={activeTab()?.loading}
    class:private={privateMode}
    type="text"
    spellcheck="false"
    autocomplete="off"
    placeholder={privateMode ? "無痕模式：搜尋或輸入網址" : "搜尋或輸入網址"}
    aria-label="網址與搜尋"
    onfocus={() => {
      focused = true;
      inputEl?.select();
    }}
    onblur={() => (focused = false)}
    onkeydown={(e) => {
      if (e.key === "Enter") submit();
      if (e.key === "Escape") reset();
    }}
  />
  <PageActions />
</div>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 44px;
    padding: 0 8px;
    border-bottom: 1px solid var(--hairline);
    background: var(--panel);
  }
  .nav {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 6px;
    color: var(--text-secondary);
  }
  .nav:hover {
    background: var(--hover);
    color: var(--text);
  }
  .address {
    flex: 1;
    min-width: 0;
    height: 30px;
    margin: 0 4px;
    padding: 0 12px;
    border: 1px solid transparent;
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
    user-select: text;
  }
  .address:hover {
    border-color: var(--hairline);
  }
  .address:focus {
    outline: none;
    border-color: var(--accent);
  }
  /* Private mode is a state, not decoration: the quiet pill marks it. */
  .address.private {
    border-color: var(--hairline);
    border-radius: 15px;
  }
  /* Loading is a state, not decoration: a thin accent line while the
     active tab is in flight. */
  .address.loading {
    border-bottom: 2px solid var(--accent);
  }
</style>
