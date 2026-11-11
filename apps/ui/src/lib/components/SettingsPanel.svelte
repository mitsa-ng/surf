<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { isMobileDevice } from "../platform";
  import { pickSyncFile, rulesGet, rulesSet, syncAutoGet, syncAutoSet, syncExport, syncImport } from "../ipc";

  const engines = ["duckduckgo", "google", "bing"] as const;
  type Engine = (typeof engines)[number];
  const isMobile = isMobileDevice();

  let engine = $state<Engine>("duckduckgo");
  let engineLoaded = $state(false);
  let engineSaving = $state(false);
  let engineNotice = $state("");
  let engineError = $state("");
  let blocklist = $state("");
  let blocklistLoaded = $state(false);
  let blocklistNotice = $state("");
  let blocklistError = $state("");
  let syncPath = $state("");
  let syncNotice = $state("");
  let autoSyncPath = $state<string | null>(null);
  let autoSyncNotice = $state("");

  async function doExport() {
    try {
      syncPath = await syncExport();
      syncNotice = `已匯出：${syncPath}`;
    } catch (e) {
      syncNotice = String(e);
    }
  }

  async function enableAutoSync() {
    autoSyncNotice = "";
    try {
      const picked = await pickSyncFile();
      if (!picked) return;
      autoSyncPath = await syncAutoSet(picked);
      autoSyncNotice = `自動同步中：${autoSyncPath}`;
    } catch (e) {
      autoSyncNotice = String(e);
    }
  }

  async function disableAutoSync() {
    autoSyncNotice = "";
    try {
      await syncAutoSet(null);
      autoSyncPath = null;
      autoSyncNotice = "已停止自動同步";
    } catch (e) {
      autoSyncNotice = String(e);
    }
  }

  if (!isMobile) {
    void syncAutoGet()
      .then((path) => (autoSyncPath = path))
      .catch(() => (autoSyncPath = null));
  }

  async function doImport() {
    if (!syncPath.trim()) return;
    try {
      syncNotice = await syncImport(syncPath.trim());
    } catch (e) {
      syncNotice = String(e);
    }
  }

  async function loadEngine() {
    try {
      const setting = await invoke<string | null>("settings_get", { key: "search_engine" });
      if (setting && (engines as readonly string[]).includes(setting)) engine = setting as Engine;
    } catch (error) {
      engineError = `讀取失敗：${String(error)}`;
    } finally {
      engineLoaded = true;
    }
  }

  async function saveEngine(value: Engine) {
    engine = value;
    engineError = "";
    engineNotice = "";
    engineSaving = true;
    try {
      await invoke("settings_set", { key: "search_engine", value });
      engineNotice = "已儲存並套用";
    } catch (error) {
      engineError = `儲存失敗：${String(error)}`;
    } finally {
      engineSaving = false;
    }
  }

  void loadEngine();

  rulesGet()
    .then((filters) => {
      blocklist = filters.join("\n");
      blocklistLoaded = true;
    })
    .catch((error) => (blocklistError = `讀取失敗：${String(error)}`));

  async function saveBlocklist() {
    const filters = blocklist
      .split("\n")
      .map((l) => l.trim())
      .filter((l) => l && !l.startsWith("!"));
    blocklistNotice = "";
    blocklistError = "";
    try {
      const saved = await rulesSet(filters);
      blocklist = saved.join("\n");
      blocklistNotice = `已儲存 ${saved.length} 條規則並生效`;
    } catch (error) {
      blocklistError = `儲存失敗：${String(error)}`;
    }
  }
</script>

<section class="panel" aria-label="設定">
  <h2>設定</h2>
  <div class="row">
    <span>搜尋引擎</span>
    <div class="engine-options" role="group" aria-label="搜尋引擎">
      {#each engines as e}
        <button
          type="button"
          class:selected={engine === e}
          aria-pressed={engine === e}
          onclick={() => void saveEngine(e)}
          disabled={!engineLoaded || engineSaving}
        >
          {e}
        </button>
      {/each}
    </div>
  </div>
  {#if engineError}
    <p class="error" role="status" aria-live="polite">{engineError}</p>
  {:else if engineNotice}
    <p class="notice" role="status" aria-live="polite">{engineNotice}</p>
  {/if}
  <h3>同步（書籤＋瀏覽紀錄，檔案匯出/匯入）</h3>
  <div class="row">
    <button class="primary" onclick={() => doExport()}>匯出檔案</button>
    <input class="sync-path" bind:value={syncPath} type="text" placeholder="surf-sync.json 路徑" aria-label="同步檔路徑" spellcheck="false" />
    <button onclick={() => doImport()}>匯入</button>
  </div>
  {#if syncNotice}<p class="notice">{syncNotice}</p>{/if}
  {#if !isMobile}
    <div class="row">
      {#if autoSyncPath}
        <button onclick={() => disableAutoSync()}>停止自動同步</button>
        <span class="notice" role="status">自動同步中：{autoSyncPath}</span>
      {:else}
        <button onclick={() => enableAutoSync()}>啟用自動同步…</button>
        <span class="notice">選一個雲端資料夾裡的 surf-sync.json，兩台裝置指同一個檔案即可互同步</span>
      {/if}
    </div>
    {#if autoSyncNotice}<p class="notice" role="status" aria-live="polite">{autoSyncNotice}</p>{/if}
  {/if}
  <h3>封鎖清單（EasyList 語法，擋頂層導航）</h3>
  <textarea bind:value={blocklist} rows="6" spellcheck="false" aria-label="封鎖規則"></textarea>
  <div class="row">
    <button class="primary" onclick={() => saveBlocklist()} disabled={!blocklistLoaded}>儲存規則</button>
    {#if blocklistNotice}<span class="notice">{blocklistNotice}</span>{/if}
  </div>
  {#if blocklistError}<p class="error" role="status" aria-live="polite">{blocklistError}</p>{/if}
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
  .row > span {
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
  .error {
    color: #b3261e;
    font-size: 12px;
  }
  .engine-options {
    display: flex;
    gap: 4px;
  }
  .engine-options button {
    height: 32px;
    padding: 0 8px;
    border: 1px solid var(--hairline);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
    font-size: 12px;
  }
  .engine-options button.selected {
    background: var(--accent);
    color: #fff;
  }
</style>
