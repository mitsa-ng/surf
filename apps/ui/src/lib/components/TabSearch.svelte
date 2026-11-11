<script lang="ts">
  import { ui } from "../ui.svelte";
  import { tabs, activateTab } from "../tabs.svelte";

  let filter = $state("");
  let inputEl: HTMLInputElement | undefined = $state();

  $effect(() => {
    if (ui.tabSearchOpen) {
      filter = "";
      // Focus after the overlay mounts.
      setTimeout(() => inputEl?.focus(), 0);
    }
  });

  const results = $derived(
    tabs.list.filter((t) => {
      const q = filter.trim().toLowerCase();
      if (!q) return true;
      return (t.title ?? "").toLowerCase().includes(q) || (t.url ?? "").toLowerCase().includes(q);
    }),
  );

  function pick(id: number) {
    ui.tabSearchOpen = false;
    void activateTab(id);
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") ui.tabSearchOpen = false;
    if (e.key === "Enter" && results[0]) {
      e.preventDefault();
      pick(results[0].id);
    }
  }
</script>

{#if ui.tabSearchOpen}
  <div class="scrim" role="presentation" onclick={() => (ui.tabSearchOpen = false)}></div>
  <div class="tab-search" role="dialog" aria-label="搜尋分頁">
    <input
      bind:this={inputEl}
      bind:value={filter}
      type="text"
      placeholder="搜尋分頁"
      aria-label="搜尋分頁"
      spellcheck="false"
      onkeydown={onKeydown}
    />
    <ul>
      {#each results as t (t.id)}
        <li>
          <button
            class:active={t.id === tabs.activeId}
            onclick={() => pick(t.id)}
            title={t.url ?? ""}
          >
            <span class="title">{t.title || t.url || "新分頁"}</span>
            <span class="url">{t.url ?? ""}</span>
          </button>
        </li>
      {:else}
        <li class="empty">沒有符合的分頁</li>
      {/each}
    </ul>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 3;
  }
  .tab-search {
    position: fixed;
    top: 8px;
    right: 12px;
    z-index: 4;
    width: min(320px, 80vw);
    padding: 6px;
    background: var(--panel);
    border: 1px solid var(--hairline);
    border-radius: 8px;
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.12);
  }
  input {
    width: 100%;
    height: 30px;
    padding: 0 10px;
    border: 1px solid var(--hairline);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
    font-size: 13px;
  }
  input:focus {
    outline: none;
    border-color: var(--accent);
  }
  ul {
    list-style: none;
    margin: 6px 0 0;
    padding: 0;
    max-height: 260px;
    overflow-y: auto;
  }
  button {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
    width: 100%;
    min-height: 44px;
    padding: 5px 10px;
    border-radius: 6px;
    text-align: left;
  }
  button:hover {
    background: var(--hover);
  }
  button.active .title {
    color: var(--accent);
  }
  .title {
    font-size: 13px;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .url {
    font-size: 11px;
    color: var(--text-secondary);
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .empty {
    padding: 10px;
    color: var(--text-secondary);
    font-size: 12px;
  }
</style>
