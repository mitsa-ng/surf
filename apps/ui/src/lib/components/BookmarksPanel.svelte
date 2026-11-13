<script lang="ts">
  import { ui } from "../ui.svelte";
  import { bookmarksRemove } from "../ipc";
  import Icon from "./Icon.svelte";

  function go(url: string) {
    // Reuse the omnibox path by closing the panel and navigating.
    ui.panel = null;
    window.dispatchEvent(new CustomEvent("surf:navigate", { detail: url }));
  }

  async function remove(url: string) {
    try {
      ui.bookmarks = await bookmarksRemove(url);
    } catch (e) {
      console.error(String(e));
    }
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
        <button
          class="remove"
          onclick={() => void remove(b.url)}
          aria-label={`刪除書籤 ${b.title ?? b.url}`}
          title="刪除書籤"
        >
          <Icon name="close" />
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
  li {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .row {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    flex: 1;
    min-width: 0;
    width: 100%;
    min-height: 44px;
    padding: 5px 10px;
    border-radius: 6px;
    text-align: left;
  }
  .row:hover,
  .row:active {
    background: var(--hover);
  }
  .title {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
  }
  .url {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11px;
    color: var(--text-secondary);
  }
  .remove {
    flex: none;
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 6px;
    color: var(--text-secondary);
  }
  .remove:hover {
    color: var(--text);
    background: var(--hover);
  }
  .remove :global(svg) {
    width: 11px;
    height: 11px;
  }
  .empty {
    color: var(--text-secondary);
    padding: 8px 0;
    font-size: 13px;
  }
</style>
