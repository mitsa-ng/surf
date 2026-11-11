<script lang="ts">
  import { tabs, navigateActive, historyAction } from "../tabs.svelte";
  import { ui } from "../ui.svelte";
  import { displayAddress } from "../format";
  import Icon from "./Icon.svelte";
  import PageActions from "./PageActions.svelte";

  let input = $state("");
  let focused = $state(false);
  let highlight = $state(-1);
  let inputEl: HTMLInputElement | undefined = $state();
  const privateMode = $derived(activeTab()?.incognito ?? false);

  // Refill from the model whenever the address changes and we are not editing.
  $effect(() => {
    const shown = displayAddress(activeTab()?.url ?? null);
    if (!focused) input = shown;
  });

  function activeTab() {
    return tabs.list.find((t) => t.id === tabs.activeId) ?? null;
  }

  export function focusAddressBar() {
    inputEl?.focus();
    inputEl?.select();
  }

  type Suggestion = { url: string; title: string; source: "bookmark" | "history" };

  const suggestions = $derived.by(() => {
    const q = input.trim().toLowerCase();
    if (!q || !focused) return [] as Suggestion[];
    const seen = new Set<string>();
    const out: Suggestion[] = [];
    // Bookmarks first, then recent history; both live in the ui store.
    const hits = (rows: { url: string; title: string | null }[], source: Suggestion["source"]) => {
      for (const r of rows) {
        if (seen.has(r.url)) continue;
        if (!r.url.toLowerCase().includes(q) && !(r.title ?? "").toLowerCase().includes(q)) continue;
        seen.add(r.url);
        out.push({ url: r.url, title: r.title || r.url, source });
      }
    };
    hits(ui.bookmarks, "bookmark");
    hits(ui.history, "history");
    return out.slice(0, 6);
  });

  $effect(() => {
    if (highlight >= suggestions.length) highlight = -1;
  });

  function closeSuggestions() {
    highlight = -1;
    inputEl?.blur();
  }

  function submit() {
    const picked = highlight >= 0 ? suggestions[highlight]?.url : input;
    if (picked?.trim()) navigateActive(picked);
    closeSuggestions();
  }

  function reset() {
    input = displayAddress(activeTab()?.url ?? null);
    highlight = -1;
    inputEl?.blur();
  }
</script>

<div class="toolbar">
  <button class="nav" onclick={() => historyAction("back")} aria-label="上一頁" title="上一頁">
    <Icon name="back" />
  </button>
  <button class="nav" onclick={() => historyAction("forward")} aria-label="下一頁" title="下一頁">
    <Icon name="forward" />
  </button>
  <button class="nav" onclick={() => historyAction("reload")} aria-label="重新載入" title="重新載入">
    <Icon name="reload" />
  </button>
  <div class="address-wrap">
    <input
      bind:this={inputEl}
      bind:value={input}
      class="address"
      class:loading={activeTab()?.loading}
      class:private={privateMode}
      type="text"
      spellcheck="false"
      autocomplete="off"
      placeholder={privateMode ? "無痕模式：搜尋或輸入網址" : "搜尋或輸入網址"}
      aria-label="網址與搜尋"
      role="combobox"
      aria-expanded={suggestions.length > 0}
      aria-controls="address-suggestions"
      onfocus={() => {
        focused = true;
        inputEl?.select();
      }}
      onblur={() => (focused = false)}
      oninput={() => (highlight = -1)}
      onkeydown={(e) => {
        if (e.key === "ArrowDown" && suggestions.length > 0) {
          e.preventDefault();
          highlight = Math.min(highlight + 1, suggestions.length - 1);
        } else if (e.key === "ArrowUp" && suggestions.length > 0) {
          e.preventDefault();
          highlight = highlight <= 0 ? -1 : highlight - 1;
        } else if (e.key === "Enter") {
          submit();
        } else if (e.key === "Escape") {
          if (highlight >= 0) highlight = -1;
          else reset();
        }
      }}
    />
    {#if suggestions.length > 0}
      <ul class="suggest" id="address-suggestions" role="listbox" aria-label="建議">
        {#each suggestions as s, i (s.url)}
          <li>
            <button
              type="button"
              role="option"
              aria-selected={i === highlight}
              class:selected={i === highlight}
              onmousedown={(e) => {
                e.preventDefault();
                navigateActive(s.url);
                closeSuggestions();
              }}
            >
              <span class="source" aria-label={s.source === "bookmark" ? "書籤" : "歷史"}>
                {s.source === "bookmark" ? "★" : "↺"}
              </span>
              <span class="title">{s.title}</span>
              <span class="url">{s.url}</span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
  <PageActions />
</div>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 44px;
    padding: 0 8px;
    border-bottom: 1px solid var(--hairline);
    background: var(--panel);
  }
  .nav {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 6px;
    color: var(--text-secondary);
  }
  .nav:hover {
    background: var(--hover);
    color: var(--text);
  }
  .address-wrap {
    position: relative;
    flex: 1;
    min-width: 0;
    margin: 0 4px;
  }
  .address {
    width: 100%;
    height: 30px;
    padding: 0 12px;
    border: 1px solid transparent;
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
    user-select: text;
  }
  .address:hover {
    border-color: var(--hairline);
  }
  .address:focus {
    outline: none;
    border-color: var(--accent);
  }
  /* Loading is a state, not decoration: a thin accent line while the
     active tab is in flight. */
  .address.loading {
    border-bottom: 2px solid var(--accent);
  }
  /* Private mode is a state, not decoration: the quiet pill marks it. */
  .address.private {
    border-color: var(--hairline);
    border-radius: 15px;
  }
  .suggest {
    position: absolute;
    top: calc(100% + 2px);
    left: 0;
    right: 0;
    z-index: 6;
    margin: 0;
    padding: 4px;
    list-style: none;
    background: var(--panel);
    border: 1px solid var(--hairline);
    border-radius: 8px;
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.12);
  }
  .suggest button {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: 36px;
    padding: 4px 10px;
    border-radius: 6px;
    text-align: left;
  }
  .suggest button:hover,
  .suggest button.selected {
    background: var(--hover);
  }
  .source {
    flex: none;
    width: 14px;
    color: var(--text-secondary);
    font-size: 11px;
    text-align: center;
  }
  .suggest .title {
    font-size: 13px;
    max-width: 45%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .suggest .url {
    flex: 1;
    min-width: 0;
    font-size: 11px;
    color: var(--text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
