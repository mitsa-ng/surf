<script lang="ts">
  import { ui, refreshHistory } from "../ui.svelte";
  import { historyClear, historyDelete } from "../ipc";
  import Icon from "./Icon.svelte";

  let query = $state("");
  let confirming = $state(false);
  let clearNotice = $state("");
  let searchTimer: ReturnType<typeof setTimeout> | undefined;

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
      await refreshHistory(query);
    } catch (e) {
      clearNotice = String(e);
    }
  }

  async function remove(url: string, visitedAt: number) {
    try {
      await historyDelete(url, visitedAt);
      await refreshHistory(query);
    } catch (e) {
      console.error(String(e));
    }
  }

  // Debounced search: the DB does the matching (url + title), so the
  // panel is not limited to the rows it already fetched.
  function onSearch() {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => void refreshHistory(query), 200);
  }

  // Day bucket for the date grouping (local time, like every history UI).
  function dayLabel(ts: number): string {
    const d = new Date(ts);
    const today = new Date();
    const yesterday = new Date(today);
    yesterday.setDate(today.getDate() - 1);
    const same = (a: Date, b: Date) =>
      a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
    if (same(d, today)) return "今天";
    if (same(d, yesterday)) return "昨天";
    return `${d.getFullYear()}/${d.getMonth() + 1}/${d.getDate()}`;
  }

  const groups = $derived.by(() => {
    const out: { label: string; rows: typeof ui.history }[] = [];
    for (const row of ui.history) {
      const label = dayLabel(row.visited_at);
      const last = out[out.length - 1];
      if (last && last.label === label) last.rows.push(row);
      else out.push({ label, rows: [row] });
    }
    return out;
  });
</script>

<section class="panel" aria-label="瀏覽紀錄">
  <h2>瀏覽紀錄</h2>
  <div class="controls">
    <input
      class="search"
      bind:value={query}
      oninput={onSearch}
      type="search"
      placeholder="搜尋紀錄"
      aria-label="搜尋瀏覽紀錄"
      spellcheck="false"
    />
  </div>
  <div class="controls">
    <button class="refresh" onclick={() => void refreshHistory(query)}>重新整理</button>
    <button class="clear" class:confirming onclick={() => void clearAll()}>
      {confirming ? "確定清除全部？" : "清除所有紀錄"}
    </button>
    {#if clearNotice}<span class="notice" role="status">{clearNotice}</span>{/if}
  </div>
  {#each groups as g (g.label)}
    <h3>{g.label}</h3>
    <ul>
      {#each g.rows as h (h.url + h.visited_at)}
        <li>
          <button class="row" onclick={() => go(h.url)}>
            <span class="title">{h.title ?? h.url}</span>
            <span class="url">{h.url}</span>
          </button>
          <button
            class="remove"
            onclick={() => void remove(h.url, h.visited_at)}
            aria-label={`刪除 ${h.title ?? h.url} 這筆紀錄`}
            title="刪除這筆"
          >
            <Icon name="close" />
          </button>
        </li>
      {/each}
    </ul>
  {:else}
    {#if ui.history.length === 0}
      <p class="empty">{query.trim() ? "沒有符合的紀錄。" : "沒有紀錄。"}</p>
    {/if}
  {/each}
</section>

<style>
  h2 {
    margin: 0 0 12px;
    font-size: 14px;
    font-weight: 600;
  }
  h3 {
    margin: 14px 0 4px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
  }
  .search {
    flex: 1;
    min-width: 0;
    height: 30px;
    padding: 0 10px;
    border: 1px solid var(--hairline);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
    font-size: 13px;
  }
  .search:focus {
    outline: none;
    border-color: var(--accent);
  }
  button {
    height: 30px;
    padding: 0 10px;
    border-radius: 6px;
    color: var(--text);
    font-size: 12px;
  }
  button:hover,
  button:active {
    background: var(--hover);
  }
  .clear.confirming {
    color: #b3261e;
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
    height: auto;
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
  .notice {
    color: var(--text-secondary);
    font-size: 12px;
  }
</style>
