<script lang="ts">
  import { ui, togglePanel } from "../ui.svelte";
  import Icon from "./Icon.svelte";
  import BookmarksPanel from "./BookmarksPanel.svelte";
  import HistoryPanel from "./HistoryPanel.svelte";
  import ExtensionsPanel from "./ExtensionsPanel.svelte";
  import SettingsPanel from "./SettingsPanel.svelte";
  import DownloadsPanel from "./DownloadsPanel.svelte";
  import AgentPanel from "./AgentPanel.svelte";

  function onKeydown(e: KeyboardEvent) {
    // Esc closes the drawer like every other chrome overlay (R-32).
    if (e.key === "Escape" && ui.panel !== null) {
      e.stopPropagation();
      ui.panel = null;
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if ui.panel}
  <div class="scrim" role="presentation" onclick={() => (ui.panel = null)}></div>
  <aside
    class="drawer"
    aria-label={ui.panel === "settings" ? "設定" : ui.panel === "extensions" ? "擴充功能" : "面板"}
  >
    <header>
      <button class="close" onclick={() => togglePanel(ui.panel!)} aria-label="關閉面板">
        <Icon name="close" />
      </button>
    </header>
    {#if ui.panel === "bookmarks"}
      <BookmarksPanel />
    {:else if ui.panel === "history"}
      <HistoryPanel />
    {:else if ui.panel === "extensions"}
      <ExtensionsPanel />
    {:else if ui.panel === "settings"}
      <SettingsPanel />
    {:else if ui.panel === "downloads"}
      <DownloadsPanel />
    {:else if ui.panel === "agent"}
      <AgentPanel />
    {/if}
  </aside>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 30;
    background: rgba(0, 0, 0, 0.2);
  }
  .drawer {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: 31;
    width: min(360px, 90vw);
    overflow-y: auto;
    padding: 8px 16px 16px;
    background: var(--panel);
    border-left: 1px solid var(--hairline);
  }
  /* Mobile: the drawer becomes a bottom sheet above the bottom bar —
     top must be cleared or the base rule's top:0 keeps it full-height
     and it swallows the scrim. */
  :global(.mobile) .drawer {
    left: 0;
    right: 0;
    bottom: calc(88px + env(safe-area-inset-bottom));
    top: auto;
    max-height: 78vh;
    width: auto;
    border-left: none;
    border-top: 1px solid var(--hairline);
    border-radius: 16px 16px 0 0;
  }
  header {
    display: flex;
    justify-content: flex-end;
  }
  .close {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 6px;
    color: var(--text-secondary);
  }
  .close :global(svg) {
    width: 13px;
    height: 13px;
  }
  .close:hover {
    background: var(--hover);
    color: var(--text);
  }
</style>
