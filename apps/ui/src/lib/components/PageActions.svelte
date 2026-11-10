<script lang="ts">
  import { ui, toggleBookmark, isBookmarked, togglePanel } from "../ui.svelte";
  import { tabs } from "../tabs.svelte";
  import Icon from "./Icon.svelte";

  const active = $derived(tabs.list.find((t) => t.id === tabs.activeId) ?? null);
</script>

<div class="actions">
  <button
    class="star"
    class:on={isBookmarked(active?.url ?? null)}
    onclick={() => toggleBookmark()}
    aria-label={isBookmarked(active?.url ?? null) ? "取消書籤" : "加入書籤"}
    title="書籤"
  >
    <Icon name="star" />
  </button>
  <button onclick={() => togglePanel("bookmarks")} aria-label="書籤列表" title="書籤">書籤</button>
  <button onclick={() => togglePanel("history")} aria-label="瀏覽紀錄" title="歷史">歷史</button>
  <button onclick={() => togglePanel("extensions")} aria-label="擴充功能" title="擴充功能">擴充</button>
  <button onclick={() => togglePanel("settings")} aria-label="設定" title="設定">設定</button>
</div>

<style>
  .actions {
    display: flex;
    align-items: center;
    gap: 2px;
    margin-left: 4px;
    flex: none;
  }
  button {
    height: 30px;
    padding: 0 8px;
    border-radius: 6px;
    color: var(--text-secondary);
    font-size: 12px;
  }
  button:hover {
    background: var(--hover);
    color: var(--text);
  }
  .star.on {
    color: var(--accent);
  }
</style>
