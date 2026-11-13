<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { isMobileDevice } from "../platform";
  import { pickSyncFile, rulesGet, rulesSet, syncAutoGet, syncAutoSet, syncExport, syncImport } from "../ipc";
  import { agentLlmConfigGet, agentLlmConfigSet } from "../agent.svelte";
  import { refreshPasswords, deletePassword, ui as uiState } from "../ui.svelte";

  const engines = ["duckduckgo", "google", "bing", "custom"] as const;
  type Engine = (typeof engines)[number];
  const isMobile = isMobileDevice();

  let engine = $state<Engine>("duckduckgo");
  let engineLoaded = $state(false);
  let engineSaving = $state(false);
  let engineNotice = $state("");
  let engineError = $state("");
  let customTemplate = $state("");
  let suggestLive = $state(true);
  let blocklist = $state("");
  let blocklistLoaded = $state(false);
  let blocklistNotice = $state("");
  let blocklistError = $state("");
  let syncPath = $state("");
  let syncNotice = $state("");
  let autoSyncPath = $state<string | null>(null);
  let autoSyncNotice = $state("");
  let llmBase = $state("");
  let llmModel = $state("");
  let llmKey = $state("");
  let llmHasKey = $state(false);
  let llmNotice = $state("");

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
      const custom = await invoke<string | null>("settings_get", { key: "search_engine_custom" });
      customTemplate = custom ?? "";
      const liveValue = await invoke<string | null>("settings_get", { key: "suggest_live" });
      suggestLive = liveValue !== "0";
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
      if (value === "custom" && !customTemplateValid) {
        engineError = "自訂引擎需要含 {q} 的網址範本，例如 https://kagi.com/search?q={q}";
        return;
      }
      await invoke("settings_set", { key: "search_engine", value });
      if (value === "custom") {
        await invoke("settings_set", { key: "search_engine_custom", value: customTemplate.trim() });
      }
      engineNotice = "已儲存並套用";
    } catch (error) {
      engineError = `儲存失敗：${String(error)}`;
    } finally {
      engineSaving = false;
    }
  }

  const customTemplateValid = $derived(
    customTemplate.trim().includes("{q}") && !customTemplate.trim().includes(" ") && /^https?:\/\//.test(customTemplate.trim()),
  );

  async function saveCustom() {
    if (!customTemplateValid) {
      engineError = "自訂引擎需要含 {q} 的網址範本（不含空格），例如 https://kagi.com/search?q={q}";
      return;
    }
    await saveEngine("custom");
  }

  async function saveSuggestLive(value: boolean) {
    suggestLive = value;
    try {
      await invoke("settings_set", { key: "suggest_live", value: value ? "1" : "0" });
    } catch (error) {
      console.error(String(error));
    }
  }

  void loadEngine();

  rulesGet()
    .then((filters) => {
      blocklist = filters.join("\n");
      blocklistLoaded = true;
    })
    .catch((error) => (blocklistError = `讀取失敗：${String(error)}`));

  void refreshPasswords();

  function loadLlm() {
    void agentLlmConfigGet()
      .then((cfg) => {
        llmBase = cfg.baseUrl;
        llmModel = cfg.model;
        llmHasKey = cfg.hasKey;
      })
      .catch(() => {});
  }
  loadLlm();

  async function saveLlm(clearKey = false) {
    llmNotice = "";
    try {
      await agentLlmConfigSet(llmBase, llmModel, clearKey ? null : llmKey || null, clearKey);
      llmKey = "";
      const cfg = await agentLlmConfigGet();
      llmBase = cfg.baseUrl;
      llmModel = cfg.model;
      llmHasKey = cfg.hasKey;
      llmNotice =
        clearKey
          ? "已清除金鑰；Agent 回到示範模式"
          : cfg.baseUrl && cfg.model && cfg.hasKey
            ? "已儲存：Agent 面板會改用 LLM 回應"
            : "已儲存（尚未完整設定，Agent 續用示範模式）";
    } catch (e) {
      llmNotice = String(e);
    }
  }

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
  {#if engine === "custom" || customTemplate.trim()}
    <div class="row">
      <input
        class="sync-path"
        bind:value={customTemplate}
        type="text"
        placeholder={"https://example.com/search?q={q}"}
        aria-label="自訂搜尋引擎範本"
        spellcheck="false"
        disabled={!engineLoaded}
      />
      <button onclick={() => void saveCustom()} disabled={!customTemplateValid || engineSaving}>儲存範本</button>
    </div>
  {/if}
  <div class="row">
    <label class="check">
      <input type="checkbox" checked={suggestLive} onchange={(e) => void saveSuggestLive(e.currentTarget.checked)} />
      <span>網址列顯示搜尋引擎的即時建議</span>
    </label>
  </div>
  {#if engineError}
    <p class="error" role="status" aria-live="polite">{engineError}</p>
  {:else if engineNotice}
    <p class="notice" role="status" aria-live="polite">{engineNotice}</p>
  {/if}
  <h3>Agent（OpenAI 相容 LLM）</h3>
  <div class="row">
    <input
      class="sync-path"
      bind:value={llmBase}
      type="text"
      placeholder="https://api.openai.com/v1"
      aria-label="LLM API base URL"
      spellcheck="false"
    />
    <input
      class="sync-path short"
      bind:value={llmModel}
      type="text"
      placeholder="模型名稱"
      aria-label="LLM 模型名稱"
      spellcheck="false"
    />
  </div>
  <div class="row">
    <input
      class="sync-path"
      bind:value={llmKey}
      type="password"
      placeholder={llmHasKey ? "金鑰已儲存（留空保留）" : "API 金鑰（存於 macOS 鑰匙圈）"}
      aria-label="LLM API 金鑰"
      spellcheck="false"
      autocomplete="off"
    />
    <button class="primary" onclick={() => void saveLlm()}>儲存</button>
    {#if llmHasKey}
      <button onclick={() => void saveLlm(true)}>清除金鑰</button>
    {/if}
  </div>
  {#if llmNotice}<p class="notice" role="status" aria-live="polite">{llmNotice}</p>{/if}

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
  <h3>已儲存的密碼（macOS 鑰匙圈）</h3>
  <ul class="pw-list">
    {#each uiState.passwords as row (row.host)}
      <li>
        <span class="pw-meta">
          <span class="pw-host">{row.host}</span>
          <span class="pw-user">{row.user}</span>
        </span>
        <button onclick={() => void deletePassword(row.host)} aria-label={`刪除 ${row.host} 的密碼`}>刪除</button>
      </li>
    {:else}
      <li class="empty">尚無儲存的密碼。在登入表單送出後會詢問是否儲存。</li>
    {/each}
  </ul>

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
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }
  .check input {
    width: 15px;
    height: 15px;
    accent-color: var(--accent);
  }
  .pw-list {
    list-style: none;
    margin: 0 0 10px;
    padding: 0;
  }
  .pw-list li {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 34px;
  }
  .pw-meta {
    display: flex;
    flex-direction: column;
    gap: 1px;
    flex: 1;
    min-width: 0;
    font-size: 12px;
  }
  .pw-host {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pw-user {
    color: var(--text-secondary);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pw-list button {
    flex: none;
    height: 26px;
    padding: 0 10px;
    border-radius: 6px;
    font-size: 12px;
    color: var(--text-secondary);
  }
  .pw-list button:hover {
    background: var(--hover);
    color: var(--text);
  }
  .pw-list .empty {
    color: var(--text-secondary);
    font-size: 12px;
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
  .sync-path.short {
    flex: 0 0 130px;
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
