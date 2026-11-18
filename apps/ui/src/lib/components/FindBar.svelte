<script lang="ts">
  import { onMount } from "svelte";
  import { ui } from "../ui.svelte";
  import { tabs } from "../tabs.svelte";
  import { findInPage, onFindCount } from "../ipc";
  import Icon from "./Icon.svelte";

  let query = $state("");
  let inputEl: HTMLInputElement | undefined = $state();
  // The live match count comes from the content webview itself; it resets
  // with every new query and hides when the bar is empty (an honest 0 is
  // shown when the page has no matches).
  let count = $state<number | null>(null);

  $effect(() => {
    if (ui.findOpen) {
      query = "";
      count = null;
      setTimeout(() => inputEl?.focus(), 0);
    }
  });

  onMount(() => {
    const unlisten = onFindCount((tab, n) => {
      if (tab === tabs.activeId) count = n;
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  });

  function find(backward: boolean) {
    const id = tabs.activeId;
    if (id === null || !query.trim()) return;
    void findInPage(id, query, backward).catch((e) => console.error(String(e)));
  }

  function onKeydown(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === "Escape") {
      ui.findOpen = false;
    } else if (e.key === "Enter") {
      e.preventDefault();
      find(e.shiftKey);
    }
  }

  // Escape from an unfocused page closes the bar via App.svelte's window
  // handler; the input's own handler covers the focused case.


  function onInput() {
    count = null;
    find(false);
  }
</script>

{#if ui.findOpen}
  <div class="find-bar" role="search" aria-label="頁內搜尋">
    <input
      bind:this={inputEl}
      bind:value={query}
      oninput={onInput}
      onkeydown={onKeydown}
      type="text"
      placeholder="頁內搜尋"
      aria-label="搜尋文字"
      spellcheck="false"
    />
    {#if query.trim()}
      <span class="count" role="status" aria-live="polite">{count === null ? "…" : String(count)}</span>
    {/if}
    <button onclick={() => find(true)} aria-label="上一個符合" title="上一個（Shift+Enter）">
      <Icon name="chevronUp" />
    </button>
    <button onclick={() => find(false)} aria-label="下一個符合" title="下一個（Enter）">
      <Icon name="chevronDown" />
    </button>
    <button class="close" onclick={() => (ui.findOpen = false)} aria-label="關閉搜尋" title="關閉（Esc）">
      <Icon name="close" />
    </button>
  </div>
{/if}

<style>
  .find-bar {
    position: absolute;
    /* Chrome floats the find bar under the omnibox, clear of the page. */
    top: 91px;
    right: 12px;
    z-index: 32;
    display: flex;
    align-items: center;
    gap: 2px;
    height: 36px;
    padding: 0 5px;
    background: var(--toolbar-surface);
    border-radius: 10px;
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.18);
  }
  /* Mobile: the bar floats just above the bottom chrome. */
  :global(.mobile) .find-bar {
    top: auto;
    right: 8px;
    left: 8px;
    bottom: calc(92px + env(safe-area-inset-bottom));
  }
  :global(.mobile) .find-bar input {
    flex: 1;
    width: auto;
  }
  input {
    width: 200px;
    height: 26px;
    padding: 0 8px;
    border: 1px solid var(--hairline);
    border-radius: 6px;
    background: var(--field);
    color: var(--text);
    font: inherit;
    font-size: 12px;
  }
  input:focus {
    outline: none;
    border-color: var(--accent);
  }
  .count {
    min-width: 24px;
    text-align: center;
    font-size: 12px;
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
  }
  button {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 6px;
    color: var(--text-secondary);
  }
  button:hover {
    background: var(--hover);
    color: var(--text);
  }
</style>
