<script lang="ts">
  import { ui, togglePanel } from "../ui.svelte";
  import Icon from "./Icon.svelte";
  import BookmarksPanel from "./BookmarksPanel.svelte";
  import HistoryPanel from "./HistoryPanel.svelte";
  import SettingsPanel from "./SettingsPanel.svelte";
</script>

{#if ui.panel}
  <div class="scrim" role="presentation" onclick={() => (ui.panel = null)}></div>
  <aside class="drawer" aria-label="面板">
    <header>
      <button class="close" onclick={() => togglePanel(ui.panel!)} aria-label="關閉面板">
        <Icon name="close" />
      </button>
    </header>
    {#if ui.panel === "bookmarks"}
      <BookmarksPanel />
    {:else if ui.panel === "history"}
      <HistoryPanel />
    {:else if ui.panel === "settings"}
      <SettingsPanel />
    {:else}
      <SettingsPanel />
    {/if}
  </aside>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.2);
  }
  .drawer {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(360px, 90vw);
    overflow-y: auto;
    padding: 8px 16px 16px;
    background: var(--panel);
    border-left: 1px solid var(--hairline);
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
