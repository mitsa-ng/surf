<script lang="ts">
  import { ui } from "../ui.svelte";
  import Favicon from "./Favicon.svelte";

  function go(url: string) {
    window.dispatchEvent(new CustomEvent("surf:navigate", { detail: url }));
  }

  function label(b: { url: string; title: string | null }): string {
    if (b.title) return b.title;
    try {
      return new URL(b.url).hostname.replace(/^www\./, "");
    } catch {
      return b.url;
    }
  }
</script>

<nav class="bookmarks-bar" aria-label="書籤列">
  {#each ui.bookmarks as b (b.url)}
    <button type="button" onclick={() => go(b.url)} title={b.title || b.url}>
      <Favicon url={b.url} size={14} />
      <span class="label">{label(b)}</span>
    </button>
  {:else}
    <span class="empty">尚無書籤；點網址列右側的星號加入</span>
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
    background: var(--toolbar-surface);
    border-top: 1px solid var(--hairline, rgba(0, 0, 0, 0.08));
  }
  .bookmarks-bar::-webkit-scrollbar {
    display: none;
  }
  button {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: none;
    height: 24px;
    padding: 0 8px;
    border-radius: 6px;
    color: var(--text-secondary);
    font-size: 12px;
    max-width: 180px;
  }
  .label {
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
