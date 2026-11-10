<script lang="ts">
  import { ui, refreshHistory } from "../ui.svelte";

  function go(url: string) {
    ui.panel = null;
    window.dispatchEvent(new CustomEvent("surf:navigate", { detail: url }));
  }
</script>

<section class="panel" aria-label="瀏覽紀錄">
  <h2>瀏覽紀錄</h2>
  <button class="refresh" onclick={() => refreshHistory()}>重新整理</button>
  <ul>
    {#each ui.history as h (h.url + h.visited_at)}
      <li>
        <button class="row" onclick={() => go(h.url)}>
          <span class="title">{h.title ?? h.url}</span>
          <span class="url">{h.url}</span>
        </button>
      </li>
    {:else}
      <li class="empty">沒有紀錄。</li>
    {/each}
  </ul>
</section>

<style>
  h2 {
    margin: 0 0 12px;
    font-size: 14px;
    font-weight: 600;
  }
  .refresh {
    height: 28px;
    padding: 0 10px;
    margin-bottom: 8px;
    border-radius: 6px;
    font-size: 12px;
    color: var(--text-secondary);
  }
  .refresh:hover {
    background: var(--hover);
    color: var(--text);
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
