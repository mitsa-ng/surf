<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import ExtensionsPanel from "./ExtensionsPanel.svelte";

  const engines = ["duckduckgo", "google", "bing"] as const;
  type Engine = (typeof engines)[number];

  let engine = $state<Engine>("duckduckgo");
  let loaded = false;

  $effect(() => {
    if (!loaded) return;
    void invoke("settings_set", { key: "search_engine", value: engine }).catch((e) => console.error(String(e)));
  });

  invoke<{ key: string; value: string } | null>("settings_get", { key: "search_engine" })
    .then((v) => {
      if (v && (engines as readonly string[]).includes(v.value)) engine = v.value as Engine;
      loaded = true;
    })
    .catch(() => {
      loaded = true;
    });
</script>

<section class="panel" aria-label="設定">
  <h2>設定</h2>
  <div class="row">
    <label for="engine">搜尋引擎</label>
    <select id="engine" bind:value={engine}>
      {#each engines as e}
        <option value={e}>{e}</option>
      {/each}
    </select>
  </div>
  <ExtensionsPanel />
</section>

<style>
  h2 {
    margin: 0 0 12px;
    font-size: 14px;
    font-weight: 600;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 16px;
  }
  label {
    font-size: 13px;
  }
  select {
    height: 32px;
    border: 1px solid var(--hairline);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
  }
</style>
