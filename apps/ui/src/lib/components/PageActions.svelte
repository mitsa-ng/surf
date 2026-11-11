<script lang="ts">
  import { ui, togglePanel } from "../ui.svelte";
  import Icon from "./Icon.svelte";

  // Panel buttons follow Chrome's shape: 28px icon-only controls, the
  // open panel marked by the same hover fill so the state is visible
  // without adding a second accent color.
  const panels = [
    { name: "downloads", icon: "download", label: "下載項目" },
    { name: "history", icon: "history", label: "瀏覽紀錄" },
    { name: "bookmarks", icon: "list", label: "書籤" },
    { name: "extensions", icon: "puzzle", label: "擴充功能" },
    { name: "settings", icon: "settings", label: "設定" },
  ] as const;
</script>

<div class="actions">
  {#each panels as p (p.name)}
    <button
      class:on={ui.panel === p.name}
      onclick={() => togglePanel(p.name)}
      aria-label={p.label}
      aria-pressed={ui.panel === p.name}
      title={p.label}
    >
      <Icon name={p.icon} />
    </button>
  {/each}
</div>

<style>
  .actions {
    display: flex;
    align-items: center;
    gap: 2px;
    flex: none;
  }
  button {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 6px;
    color: var(--text-secondary);
  }
  button:hover,
  button.on {
    background: var(--hover);
    color: var(--text);
  }
</style>
