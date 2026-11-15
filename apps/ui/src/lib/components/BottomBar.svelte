<script lang="ts">
  import { tabs, navigateActive, historyAction, openIncognito, reopenClosed } from "../tabs.svelte";
  import { ui, toggleFind, togglePanel, searchEngineLabel } from "../ui.svelte";
  import { displayAddress } from "../format";
  import { suggest, tabZoom } from "../ipc";
  import Icon from "./Icon.svelte";

  let input = $state("");
  let focused = $state(false);
  let live = $state<string[]>([]);
  let suggestTimer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    const shown = displayAddress(activeTab()?.url ?? null);
    if (!focused) input = shown;
  });

  function activeTab() {
    return tabs.list.find((t) => t.id === tabs.activeId) ?? null;
  }

  function submit() {
    if (input.trim()) navigateActive(input);
    (document.activeElement as HTMLElement)?.blur();
  }

  function menuAction(run: () => void) {
    ui.mobileMenu = false;
    run();
  }

  function zoom(mode: "in" | "out" | "reset") {
    if (tabs.activeId !== null) void tabZoom(tabs.activeId, mode).catch(console.error);
  }

  // Engine suggestions arrive async (debounced); local rows are always
  // present so the dropdown never waits on the network.
  $effect(() => {
    const q = input.trim();
    if (!q || !focused || !ui.suggestLive) {
      live = [];
      return;
    }
    clearTimeout(suggestTimer);
    suggestTimer = setTimeout(() => {
      void suggest(q)
        .then((rows) => (live = rows))
        .catch(() => (live = []));
    }, 150);
  });

  type Entry = { kind: "search"; q: string } | { kind: "url"; url: string; title: string; source: "bookmark" | "history" };

  const suggestions = $derived.by(() => {
    const q = input.trim();
    if (!q || !focused) return [] as Entry[];
    const needle = q.toLowerCase();
    const seen = new Set<string>();
    const out: Entry[] = [{ kind: "search", q }];
    for (const s of live) {
      if (s.toLowerCase() === needle || seen.has(s)) continue;
      seen.add(s);
      out.push({ kind: "search", q: s });
    }
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

  // The dropdown renders above the native content views: report its
  // visibility so App.svelte can lift the main webview (same overlay
  // contract as the tab sheet and panels).
  $effect(() => {
    ui.omniOpen = focused && suggestions.length > 0;
  });

  function pick(entry: Entry) {
    focused = false;
    if (entry.kind === "search") navigateActive(entry.q);
    else navigateActive(entry.url);
    (document.activeElement as HTMLElement)?.blur();
  }
</script>

{#if focused && suggestions.length > 0}
  <div class="suggestions" role="listbox" aria-label="建議">
    {#each suggestions as entry, i (i)}
      <button
        class="sug-row"
        role="option"
        aria-selected={i === 0}
        onpointerdown={(e) => {
          // Android moves focus on touchstart, so the input blur (which
          // tears the dropdown down) fires before the synthesized
          // mousedown/click. pointerdown is the only event that still
          // lands while the row is attached; the desktop toolbar can
          // keep mousedown because a mouse blurs only after mousedown.
          e.preventDefault();
          pick(entry);
        }}
      >
        {#if entry.kind === "search"}
          <span class="src"><Icon name="search" /></span>
          <span class="t">{entry.q}</span>
          <span class="u">{searchEngineLabel()} 搜尋</span>
        {:else}
          <span class="src"><Icon name={entry.source === "bookmark" ? "star" : "history"} /></span>
          <span class="t">{entry.title}</span>
          <span class="u">{entry.url}</span>
        {/if}
      </button>
    {/each}
  </div>
{/if}

{#if ui.mobileMenu}
  <div class="scrim" role="presentation" onclick={() => (ui.mobileMenu = false)}></div>
  <div class="menu" role="menu" aria-label="更多功能">
    <button role="menuitem" onclick={() => menuAction(() => toggleFind())}>頁內搜尋</button>
    <button role="menuitem" onclick={() => menuAction(() => togglePanel("bookmarks"))}>書籤</button>
    <button role="menuitem" onclick={() => menuAction(() => togglePanel("history"))}>瀏覽紀錄</button>
    <button role="menuitem" onclick={() => menuAction(() => togglePanel("downloads"))}>下載項目</button>
    <button role="menuitem" onclick={() => menuAction(() => togglePanel("extensions"))}>擴充功能</button>
    <button role="menuitem" onclick={() => menuAction(() => togglePanel("settings"))}>設定</button>
    <button role="menuitem" onclick={() => menuAction(() => togglePanel("agent"))}>Agent</button>
    <button role="menuitem" onclick={() => menuAction(() => zoom("in"))}>放大</button>
    <button role="menuitem" onclick={() => menuAction(() => zoom("out"))}>縮小</button>
    <button role="menuitem" onclick={() => menuAction(() => zoom("reset"))}>實際大小</button>
    <button role="menuitem" onclick={() => menuAction(() => void openIncognito())}>開新無痕分頁</button>
    <button role="menuitem" onclick={() => menuAction(() => void reopenClosed())}>重開已關閉分頁</button>
    <button role="menuitem" onclick={() => menuAction(() => (ui.help = "shortcuts"))}>鍵盤快速鍵</button>
    <button role="menuitem" onclick={() => menuAction(() => (ui.help = "about"))}>關於 Surf</button>
  </div>
{/if}

<nav class="bottom-bar">
  <button
    class="tabs-button"
    onclick={() => (tabs.sheetOpen = !tabs.sheetOpen)}
    aria-label="分頁列表"
    aria-expanded={tabs.sheetOpen}
  >
    <Icon name="tabs" />
    <span class="count">{tabs.list.length}</span>
  </button>
  <button class="nav" disabled={!activeTab()?.canBack} onclick={() => historyAction("back")} aria-label="上一頁">
    <Icon name="back" />
  </button>
  <button class="nav" disabled={!activeTab()?.canForward} onclick={() => historyAction("forward")} aria-label="下一頁">
    <Icon name="forward" />
  </button>
  <input
    class="address"
    bind:value={input}
    type="text"
    enterkeyhint="go"
    spellcheck="false"
    autocomplete="off"
    placeholder="搜尋或輸入網址"
    aria-label="網址與搜尋"
    onfocus={() => (focused = true)}
    onblur={() => (focused = false)}
    onkeydown={(e) => {
      if (e.key === "Enter") submit();
    }}
  />
  <button class="nav" onclick={() => historyAction("reload")} aria-label="重新載入">
    <Icon name="reload" />
  </button>
  <button
    class="nav"
    onclick={() => (ui.mobileMenu = !ui.mobileMenu)}
    aria-label="更多功能"
    aria-expanded={ui.mobileMenu}
  >
    <Icon name="list" />
  </button>
</nav>

<style>
  /* Mobile chrome sits at the bottom with thumb-reachable 44px targets. */
  .bottom-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px max(8px, env(safe-area-inset-left)) calc(28px + env(safe-area-inset-bottom))
      max(8px, env(safe-area-inset-right));
    border-top: 1px solid var(--hairline);
    background: var(--panel);
  }
  .tabs-button,
  .nav {
    position: relative;
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: 8px;
    color: var(--text);
    flex: none;
  }
  .tabs-button:active,
  .nav:active {
    background: var(--hover);
  }
  .count {
    position: absolute;
    bottom: 4px;
    right: 4px;
    min-width: 14px;
    height: 14px;
    padding: 0 3px;
    border-radius: 7px;
    background: var(--accent);
    color: #fff;
    font-size: 10px;
    line-height: 14px;
    text-align: center;
  }
  .address {
    flex: 1;
    min-width: 0;
    height: 44px;
    padding: 0 12px;
    border: 1px solid transparent;
    border-radius: 8px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
    font-size: 14px;
    user-select: text;
  }
  .address:focus {
    outline: none;
    border-color: var(--accent);
  }
  .suggestions {
    position: fixed;
    left: 8px;
    right: 8px;
    bottom: calc(92px + env(safe-area-inset-bottom));
    z-index: 29;
    display: flex;
    flex-direction: column;
    max-height: 46vh;
    overflow-y: auto;
    padding: 4px;
    border-radius: 12px;
    background: var(--panel);
    box-shadow: 0 10px 34px rgba(0, 0, 0, 0.24);
  }
  .sug-row {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 44px;
    padding: 0 12px;
    border-radius: 8px;
    text-align: left;
    color: var(--text);
  }
  .sug-row:active {
    background: var(--hover);
  }
  .sug-row .src {
    flex: none;
    display: grid;
    place-items: center;
    color: var(--text-secondary);
  }
  .sug-row .src :global(svg) {
    width: 14px;
    height: 14px;
  }
  .sug-row .t {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: 14px;
  }
  .sug-row .u {
    flex: none;
    max-width: 40%;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: 11px;
    color: var(--text-secondary);
  }
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 30;
    background: rgba(0, 0, 0, 0.24);
  }
  .menu {
    position: fixed;
    right: 8px;
    bottom: calc(96px + env(safe-area-inset-bottom));
    left: 8px;
    z-index: 31;
    display: flex;
    flex-direction: column;
    max-height: 60vh;
    overflow-y: auto;
    padding: 6px;
    background: var(--panel);
    border-radius: 14px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.24);
  }
  .menu button {
    min-height: 44px;
    padding: 0 14px;
    border-radius: 9px;
    text-align: left;
    font-size: 14px;
    color: var(--text);
  }
  .menu button:active {
    background: var(--hover);
  }
</style>
