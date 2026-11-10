<script lang="ts">
  import { ui, uninstall, openPopup, proposeInstall, confirmInstall, cancelInstall } from "../ui.svelte";
  import Icon from "./Icon.svelte";

</script>

<section class="panel" aria-label="擴充功能">
  <h2>擴充功能</h2>

  {#if ui.pendingInstall}
    <div class="dialog" role="dialog" aria-modal="true" aria-label="安裝擴充功能">
      <p><strong>{ui.pendingInstall.name}</strong> {ui.pendingInstall.version} 要求以下權限：</p>
      <ul>
        {#each ui.pendingInstall.permissions as perm}
          <li>{perm}</li>
        {/each}
      </ul>
      <div class="actions">
        <button class="primary" onclick={() => confirmInstall()}>安裝並授予</button>
        <button onclick={() => cancelInstall()}>取消</button>
      </div>
    </div>
  {/if}

  <form
    class="install"
    onsubmit={(e) => {
      e.preventDefault();
      void proposeInstall();
    }}
  >
    <input
      bind:value={ui.installPath}
      type="text"
      placeholder="/path/to/extension 目錄"
      aria-label="擴充功能目錄路徑"
      spellcheck="false"
    />
    <button type="submit">安裝</button>
  </form>
  {#if ui.installError}<p class="error">{ui.installError}</p>{/if}
  {#if ui.installNotice}<p class="notice">{ui.installNotice}</p>{/if}

  <ul class="list">
    {#each ui.extensions as ext (ext.id)}
      <li>
        <span class="name">{ext.name} <span class="version">{ext.version}</span></span>
        <span class="spacer"></span>
        <button onclick={() => openPopup(ext.id)} aria-label={`開啟 ${ext.name} popup`}>開啟</button>
        <button onclick={() => uninstall(ext.id)} aria-label={`移除 ${ext.name}`}>移除</button>
      </li>
    {:else}
      <li class="empty">尚未安裝擴充功能。輸入含 manifest.json 的目錄路徑安裝。</li>
    {/each}
  </ul>
</section>

<style>
  h2 {
    margin: 0 0 12px;
    font-size: 14px;
    font-weight: 600;
  }
  .install {
    display: flex;
    gap: 8px;
  }
  input {
    flex: 1;
    min-width: 0;
    height: 32px;
    padding: 0 10px;
    border: 1px solid var(--hairline);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
    user-select: text;
  }
  button {
    height: 32px;
    padding: 0 12px;
    border-radius: 6px;
    color: var(--text);
  }
  button:hover,
  button:active {
    background: var(--hover);
  }
  .primary {
    background: var(--accent);
    color: #fff;
  }
  .primary:hover {
    background: var(--accent);
    opacity: 0.9;
  }
  .list {
    list-style: none;
    margin: 12px 0 0;
    padding: 0;
  }
  .list li {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 36px;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .version {
    color: var(--text-secondary);
    font-size: 11px;
  }
  .spacer {
    flex: 1;
  }
  .empty {
    color: var(--text-secondary);
  }
  .error {
    color: #b3261e;
    margin: 8px 0 0;
    overflow-wrap: anywhere;
  }
  .notice {
    color: var(--text-secondary);
    margin: 8px 0 0;
  }
  .dialog {
    margin-bottom: 12px;
    padding: 12px;
    border: 1px solid var(--hairline);
    border-radius: 8px;
  }
  .dialog p,
  .dialog ul {
    margin: 0 0 8px;
    font-size: 12px;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
</style>
