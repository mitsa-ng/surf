<script lang="ts">
  import { ui } from "../ui.svelte";
  import Icon from "./Icon.svelte";

  function go(url: string) {
    // Reuse the omnibox path by closing the panel and navigating.
    ui.panel = null;
    window.dispatchEvent(new CustomEvent("surf:navigate", { detail: url }));
  }
</script>

<section class="panel" aria-label="書籤">
  <h2>書籤</h2>
  <ul>
    {#each ui.bookmarks as b (b.url)}
      <li>
        <button class="row" onclick={() => go(b.url)}>
          <span class="title">{b.title ?? b.url}</span>
          <span class="url">{b.url}</span>
        </button>
      </li>
    {:else}
      <li class="empty">還沒有書籤。在網址欄旁的星星加入。</li>
    {/each}
  </ul>
</section>

<style>
  h2 {
    margin: 0 0 12px;
    font-size: 14px;
    font-weight: 600;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .row {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    width: 100%;
    min-height: 44px;
    padding: 6px 10px;
    border-radius: 6px;
    text-align: left;
  }
  .row:hover {
    background: var(--hover);
  }
  .title {
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }
  .url {
    font-size: 11px;
    color: var(--text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }
  .empty {
    color: var(--text-secondary);
    padding: 8px 0;
  }
</style>
