<script lang="ts">
  import { tabs, navigateActive, historyAction, stopLoading } from "../tabs.svelte";
  import { ui, toggleBookmark, isBookmarked } from "../ui.svelte";
  import { displayAddress } from "../format";
  import Icon from "./Icon.svelte";
  import PageActions from "./PageActions.svelte";

  let input = $state("");
  let focused = $state(false);
  let highlight = $state(-1);
  let inputEl: HTMLInputElement | undefined = $state();
  const privateMode = $derived(activeTab()?.incognito ?? false);
  const loading = $derived(activeTab()?.loading ?? false);

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

  type Entry = { kind: "search"; q: string } | { kind: "url"; url: string; title: string; source: "bookmark" | "history" };

  const suggestions = $derived.by(() => {
    const q = input.trim();
    if (!q || !focused) return [] as Entry[];
    const needle = q.toLowerCase();
    const seen = new Set<string>();
    const out: Entry[] = [{ kind: "search", q }];
    const hits = (rows: { url: string; title: string | null }[], source: "bookmark" | "history") => {
      for (const r of rows) {
        if (seen.has(r.url)) continue;
        if (!r.url.toLowerCase().includes(needle) && !(r.title ?? "").toLowerCase().includes(needle)) continue;
        seen.add(r.url);
        out.push({ kind: "url", url: r.url, title: r.title || r.url, source });
      }
    };
    hits(ui.bookmarks, "bookmark");
    hits(ui.history, "history");
    return out.slice(0, 7);
  });

  $effect(() => {
    if (highlight >= suggestions.length) highlight = -1;
  });

  // Overlays need the chrome webview expanded below the toolbar; the
  // app-level effect reacts to this flag.
  $effect(() => {
    ui.suggestOpen = focused && suggestions.length > 0;
  });

  function closeSuggestions() {
    highlight = -1;
    inputEl?.blur();
  }

  function submit() {
    const picked = suggestions[highlight] ?? { kind: "search", q: input.trim() } as Entry;
    const target = picked.kind === "search" ? picked.q : picked.url;
    if (target.trim()) navigateActive(target);
    closeSuggestions();
  }

  function reset() {
    input = displayAddress(activeTab()?.url ?? null);
    highlight = -1;
    inputEl?.blur();
  }

  // Split the label around the typed query so the matched part reads as
  // matched, the way omnibox suggestions do; no innerHTML involved.
  function segments(text: string, q: string): { t: string; hit: boolean }[] {
    const lower = text.toLowerCase();
    const needle = q.toLowerCase();
    const out: { t: string; hit: boolean }[] = [];
    let from = 0;
    if (needle) {
      for (;;) {
        const at = lower.indexOf(needle, from);
        if (at === -1) break;
        if (at > from) out.push({ t: text.slice(from, at), hit: false });
        out.push({ t: text.slice(at, at + needle.length), hit: true });
        from = at + needle.length;
      }
    }
    if (from < text.length) out.push({ t: text.slice(from), hit: false });
    return out.length ? out : [{ t: text, hit: false }];
  }

  const looksLikeUrl = $derived(/^[^\s]+\.[^\s]+$/.test(input.trim()) || input.trim().startsWith("http"));
</script>

<div class="toolbar">
  <button class="nav" disabled={!activeTab()?.canBack} onclick={() => historyAction("back")} aria-label="上一頁" title="上一頁">
    <Icon name="back" />
  </button>
  <button class="nav" disabled={!activeTab()?.canForward} onclick={() => historyAction("forward")} aria-label="下一頁" title="下一頁">
    <Icon name="forward" />
  </button>
  {#if loading}
    <button class="nav" onclick={() => stopLoading()} aria-label="停止載入" title="停止載入">
      <Icon name="close" />
    </button>
  {:else}
    <button class="nav" onclick={() => historyAction("reload")} aria-label="重新載入" title="重新載入">
      <Icon name="reload" />
    </button>
  {/if}
  <div class="address-wrap">
    <span class="site" aria-hidden="true">
      <Icon name={looksLikeUrl ? "lock" : "search"} />
    </span>
    <input
      bind:this={inputEl}
      bind:value={input}
      class="address"
      class:loading={loading}
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
    <button
      class="star"
      class:on={isBookmarked(activeTab()?.url ?? null)}
      onclick={() => toggleBookmark()}
      disabled={!activeTab()?.url}
      aria-label={isBookmarked(activeTab()?.url ?? null) ? "取消書籤" : "加入書籤"}
      title="書籤"
    >
      <Icon name="star" />
    </button>
    {#if suggestions.length > 0}
      <ul class="suggest" id="address-suggestions" role="listbox" aria-label="建議">
        {#each suggestions as s, i (i)}
          <li>
            <button
              type="button"
              role="option"
              aria-selected={i === highlight}
              class:selected={i === highlight}
              onmousedown={(e) => {
                e.preventDefault();
                if (s.kind === "search") navigateActive(s.q);
                else navigateActive(s.url);
                closeSuggestions();
              }}
            >
              {#if s.kind === "search"}
                <span class="source"><Icon name="search" /></span>
                <span class="title">{#each segments(s.q, input.trim()) as seg}{#if seg.hit}<mark>{seg.t}</mark>{:else}{seg.t}{/if}{/each}</span>
                <span class="url">Google 搜尋</span>
              {:else}
                <span class="source"><Icon name={s.source === "bookmark" ? "star" : "history"} /></span>
                <span class="title">{#each segments(s.title, input.trim()) as seg}{#if seg.hit}<mark>{seg.t}</mark>{:else}{seg.t}{/if}{/each}</span>
                <span class="url">{#each segments(s.url, input.trim()) as seg}{#if seg.hit}<mark>{seg.t}</mark>{:else}{seg.t}{/if}{/each}</span>
              {/if}
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
    gap: 7px;
    height: 47px;
    padding: 0 12px;
    background: var(--toolbar-surface);
  }
  .nav {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 6px;
    color: var(--text-secondary);
    flex: none;
  }
  .nav:hover:not(:disabled) {
    background: var(--hover);
    color: var(--text);
  }
  .nav:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .address-wrap {
    position: relative;
    flex: 1;
    min-width: 0;
    margin: 0 10px;
  }
  .address {
    width: 100%;
    height: 34px;
    padding: 0 38px 0 32px;
    border: 1px solid transparent;
    border-radius: 17px;
    background: var(--field);
    color: var(--text);
    font: inherit;
    font-size: 14px;
    user-select: text;
  }
  .address:hover {
    border-color: var(--hairline);
  }
  .address:focus {
    outline: none;
    border-color: var(--accent);
    background: var(--toolbar-surface);
    box-shadow: 0 1px 8px rgba(0, 0, 0, 0.14);
  }
  /* Loading is a state, not decoration: a thin accent line while the
     active tab is in flight. */
  .address.loading {
    border-bottom: 2px solid var(--accent);
  }
  /* Private mode is a state, not decoration: the quiet pill marks it. */
  .address.private {
    border-color: var(--hairline);
  }
  .site {
    position: absolute;
    left: 10px;
    top: 50%;
    transform: translateY(-50%);
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    color: var(--text-secondary);
    pointer-events: none;
  }
  .star {
    position: absolute;
    right: 3px;
    top: 50%;
    transform: translateY(-50%);
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 8px;
    color: var(--text-secondary);
  }
  .star:hover:not(:disabled) {
    background: var(--hover);
    color: var(--text);
  }
  .star:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .star.on {
    color: var(--accent);
  }
  .suggest {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    right: 0;
    z-index: 6;
    margin: 0;
    padding: 6px;
    list-style: none;
    background: var(--toolbar-surface);
    border-radius: 16px;
    box-shadow: 0 10px 34px rgba(0, 0, 0, 0.2);
  }
  .suggest button {
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    min-height: 40px;
    padding: 4px 12px;
    border-radius: 10px;
    text-align: left;
  }
  .suggest button:hover,
  .suggest button.selected {
    background: var(--hover);
  }
  /* Chrome marks the keyboard-selected row with a bar at its left edge. */
  .suggest button.selected::before {
    content: "";
    position: absolute;
    left: 2px;
    top: 9px;
    bottom: 9px;
    width: 3px;
    border-radius: 2px;
    background: var(--accent);
  }
  .suggest mark {
    background: none;
    color: inherit;
    font-weight: 600;
  }
  .source {
    flex: none;
    display: grid;
    place-items: center;
    width: 16px;
    color: var(--text-secondary);
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
    font-size: 12px;
    color: var(--text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
