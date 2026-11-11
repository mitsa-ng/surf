<script lang="ts">
  import { ui, refreshHistory } from "../ui.svelte";
  import { historyClear } from "../ipc";

  let confirming = $state(false);
  let clearNotice = $state("");

  function go(url: string) {
    ui.panel = null;
    window.dispatchEvent(new CustomEvent("surf:navigate", { detail: url }));
  }

  // Two-step inline confirm: no native dialogs, works on every webview.
  async function clearAll() {
    if (!confirming) {
      confirming = true;
      return;
    }
    confirming = false;
    try {
      const removed = await historyClear();
      clearNotice = `已清除 ${removed} 筆紀錄`;
      await refreshHistory();
    } catch (e) {
      clearNotice = String(e);
    }
  }
</script>

<section class="panel" aria-label="瀏覽紀錄">
  <h2>瀏覽紀錄</h2>
  <div class="controls">
    <button class="refresh" onclick={() => refreshHistory()}>重新整理</button>
    <button class="clear" class:confirming onclick={() => void clearAll()}>
      {confirming ? "確定清除全部？" : "清除所有紀錄"}
    </button>
    {#if clearNotice}<span class="notice" role="status">{clearNotice}</span>{/if}
  </div>
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
  .controls {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
  }
  .refresh,
  .clear {
    height: 28px;
    padding: 0 10px;
    border-radius: 6px;
    font-size: 12px;
    color: var(--text-secondary);
  }
  .refresh:hover,
  .clear:hover {
    background: var(--hover);
    color: var(--text);
  }
  .clear.confirming {
    color: #b3261e;
  }
  .notice {
    color: var(--text-secondary);
    font-size: 12px;
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
