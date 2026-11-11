<script lang="ts">
  import { ui } from "../ui.svelte";

  function go(url: string) {
    window.dispatchEvent(new CustomEvent("surf:navigate", { detail: url }));
  }

  function label(url: string): string {
    try {
      const parsed = new URL(url);
      return parsed.hostname.replace(/^www\./, "");
    } catch {
      return url;
    }
  }
</script>

<nav class="bookmarks-bar" aria-label="書籤列">
  {#each ui.bookmarks as b (b.url)}
    <button type="button" onclick={() => go(b.url)} title={b.title || b.url}>
      {label(b.url)}
    </button>
  {:else}
    <span class="empty">尚無書籤；點網址列旁的星號加入</span>
  {/each}
</nav>

<style>
  .bookmarks-bar {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 28px;
    padding: 0 8px;
    overflow-x: auto;
    scrollbar-width: none;
    border-top: 1px solid var(--hairline, rgba(0, 0, 0, 0.08));
  }
  .bookmarks-bar::-webkit-scrollbar {
    display: none;
  }
  button {
    flex: none;
    height: 22px;
    padding: 0 8px;
    border-radius: 5px;
    color: var(--text-secondary);
    font-size: 12px;
    max-width: 160px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  button:hover {
    background: var(--hover);
    color: var(--text);
  }
  .empty {
    color: var(--text-secondary);
    font-size: 11px;
    opacity: 0.7;
    user-select: none;
  }
</style>
