<script lang="ts">
  import { ui } from "../ui.svelte";
  import { tabs } from "../tabs.svelte";
  import { findInPage } from "../ipc";

  let query = $state("");
  let inputEl: HTMLInputElement | undefined = $state();

  $effect(() => {
    if (ui.findOpen) {
      query = "";
      setTimeout(() => inputEl?.focus(), 0);
    }
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

  function onInput() {
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
    <button onclick={() => find(true)} aria-label="上一個符合" title="上一個（Shift+Enter）">↑</button>
    <button onclick={() => find(false)} aria-label="下一個符合" title="下一個（Enter）">↓</button>
    <button class="close" onclick={() => (ui.findOpen = false)} aria-label="關閉搜尋">✕</button>
  </div>
{/if}

<style>
  .find-bar {
    position: absolute;
    top: 44px;
    right: 12px;
    z-index: 5;
    display: flex;
    align-items: center;
    gap: 2px;
    height: 32px;
    padding: 0 4px;
    background: var(--panel);
    border: 1px solid var(--hairline);
    border-radius: 8px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.1);
  }
  input {
    width: 180px;
    height: 26px;
    padding: 0 8px;
    border: 1px solid var(--hairline);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
    font-size: 12px;
  }
  input:focus {
    outline: none;
    border-color: var(--accent);
  }
  button {
    width: 26px;
    height: 26px;
    border-radius: 6px;
    color: var(--text-secondary);
    font-size: 12px;
  }
  button:hover {
    background: var(--hover);
    color: var(--text);
  }
</style>
