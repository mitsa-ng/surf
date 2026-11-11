<script lang="ts">
  import { ui } from "../ui.svelte";
  import { downloadReveal } from "../ipc";

  let revealNotice = $state("");

  async function reveal(path: string) {
    revealNotice = "";
    try {
      await downloadReveal(path);
    } catch (e) {
      revealNotice = String(e);
    }
  }
</script>

<section class="panel" aria-label="下載項目">
  <h2>下載項目</h2>
  <ul>
    {#each ui.downloads as d (d.url + d.at)}
      <li>
        <div class="row">
          <div class="meta">
            <span class="name" class:failed={!d.success}>{d.filename}</span>
            <span class="path" title={d.path}>{d.path || d.url}</span>
          </div>
          {#if d.success && d.path}
            <button onclick={() => void reveal(d.path)} aria-label="在 Finder 顯示">顯示</button>
          {/if}
        </div>
      </li>
    {:else}
      <li class="empty">沒有下載項目。</li>
    {/each}
  </ul>
  {#if revealNotice}<p class="notice" role="status">{revealNotice}</p>{/if}
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
    align-items: center;
    gap: 8px;
    min-height: 44px;
    padding: 6px 10px;
    border-radius: 6px;
  }
  .row:hover {
    background: var(--hover);
  }
  .meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }
  .name {
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name.failed {
    color: #b3261e;
  }
  .path {
    font-size: 11px;
    color: var(--text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    user-select: text;
  }
  button {
    flex: none;
    height: 26px;
    padding: 0 10px;
    border-radius: 6px;
    font-size: 12px;
    color: var(--text-secondary);
  }
  button:hover {
    background: var(--bg);
    color: var(--text);
  }
  .empty {
    color: var(--text-secondary);
    padding: 8px 0;
    font-size: 13px;
  }
  .notice {
    color: var(--text-secondary);
    font-size: 12px;
  }
</style>
