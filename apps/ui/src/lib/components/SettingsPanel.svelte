<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { rulesGet, rulesSet, syncExport, syncImport } from "../ipc";
  import ExtensionsPanel from "./ExtensionsPanel.svelte";

  const engines = ["duckduckgo", "google", "bing"] as const;
  type Engine = (typeof engines)[number];

  let engine = $state<Engine>("duckduckgo");
  let loaded = false;
  let blocklist = $state("");
  let blocklistNotice = $state("");
  let syncPath = $state("");
  let syncNotice = $state("");

  async function doExport() {
    try {
      syncPath = await syncExport();
      syncNotice = `已匯出：${syncPath}`;
    } catch (e) {
      syncNotice = String(e);
    }
  }

  async function doImport() {
    if (!syncPath.trim()) return;
    try {
      syncNotice = await syncImport(syncPath.trim());
    } catch (e) {
      syncNotice = String(e);
    }
  }

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

  rulesGet()
    .then((filters) => (blocklist = filters.join("\n")))
    .catch(() => {});

  async function saveBlocklist() {
    const filters = blocklist
      .split("\n")
      .map((l) => l.trim())
      .filter((l) => l && !l.startsWith("!"));
    await rulesSet(filters).catch((e) => console.error(String(e)));
    blocklistNotice = `已儲存 ${filters.length} 條規則並生效`;
  }
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
  <h3>同步（書籤＋瀏覽紀錄，檔案匯出/匯入）</h3>
  <div class="row">
    <button class="primary" onclick={() => doExport()}>匯出檔案</button>
    <input class="sync-path" bind:value={syncPath} type="text" placeholder="surf-sync.json 路徑" aria-label="同步檔路徑" spellcheck="false" />
    <button onclick={() => doImport()}>匯入</button>
  </div>
  {#if syncNotice}<p class="notice">{syncNotice}</p>{/if}
  <h3>封鎖清單（EasyList 語法，擋頂層導航）</h3>
  <textarea bind:value={blocklist} rows="6" spellcheck="false" aria-label="封鎖規則"></textarea>
  <div class="row">
    <button class="primary" onclick={() => saveBlocklist()}>儲存規則</button>
    {#if blocklistNotice}<span class="notice">{blocklistNotice}</span>{/if}
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
  h3 {
    margin: 0 0 8px;
    font-size: 13px;
    font-weight: 600;
  }
  .sync-path {
    flex: 1;
    min-width: 0;
    height: 32px;
    padding: 0 10px;
    border: 1px solid var(--hairline);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    font: 12px ui-monospace, monospace;
    user-select: text;
  }
  textarea {
    width: 100%;
    margin-bottom: 8px;
    padding: 8px;
    border: 1px solid var(--hairline);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    font: 12px ui-monospace, monospace;
    user-select: text;
    box-sizing: border-box;
  }
  .primary {
    background: var(--accent);
    color: #fff;
  }
  .notice {
    color: var(--text-secondary);
    font-size: 12px;
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
