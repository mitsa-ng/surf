<script lang="ts">
  import { onMount } from "svelte";
  import { ui } from "../ui.svelte";
  import Icon from "./Icon.svelte";

  // The shortcuts listed here are the ones the app actually wires
  // (App.svelte onKeydown + native menus); adding a binding means adding
  // a row, nothing invented.
  const SHORTCUTS: { keys: string; label: string }[] = [
    { keys: "⌘T", label: "新分頁" },
    { keys: "⌘⇧N", label: "開新無痕分頁" },
    { keys: "⌘⇧T", label: "重開已關閉分頁" },
    { keys: "⌘W", label: "關閉分頁" },
    { keys: "⌘1 到 ⌘9", label: "跳到第 n 個分頁" },
    { keys: "⌘L", label: "聚焦網址列（全選）" },
    { keys: "⌘R", label: "重新載入" },
    { keys: "⌘F", label: "頁內搜尋（Enter 下一個，⇧Enter 上一個）" },
    { keys: "⌘⇧A", label: "搜尋分頁" },
    { keys: "⌘⇧B", label: "切換書籤列" },
    { keys: "⌘⇧J", label: "下載項目" },
    { keys: "右鍵點分頁", label: "分頁操作選單（釘選、並排、批次關閉）" },
  ];

  let version = $state("");

  onMount(async () => {
    try {
      const { getVersion, getName } = await import("@tauri-apps/api/app");
      const [v, name] = await Promise.all([getVersion(), getName()]);
      version = `${name} ${v}`;
    } catch {
      version = "";
    }
  });

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") ui.help = null;
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if ui.help}
  <div class="scrim" role="presentation" onclick={() => (ui.help = null)}></div>
  <div class="dialog" role="dialog" aria-modal="true" aria-label={ui.help === "shortcuts" ? "鍵盤快速鍵" : "關於 Surf"}>
    <header>
      <h2>{ui.help === "shortcuts" ? "鍵盤快速鍵" : "關於 Surf"}</h2>
      <button class="close" onclick={() => (ui.help = null)} aria-label="關閉">
        <Icon name="close" />
      </button>
    </header>
    {#if ui.help === "shortcuts"}
      <table>
        <tbody>
          {#each SHORTCUTS as s (s.keys + s.label)}
            <tr>
              <th scope="row">{s.label}</th>
              <td>{s.keys}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <p class="about">Surf 是安靜極簡的瀏覽器：chrome 退到內容後面，海藍只用於關鍵焦點。</p>
      {#if version}<p class="version">{version}</p>{/if}
      <p class="version">分頁與資料儲存在本機（~/Library/Application Support/com.surf.browser）。</p>
    {/if}
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: rgba(0, 0, 0, 0.24);
  }
  .dialog {
    position: fixed;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    z-index: 41;
    width: min(420px, 90vw);
    max-height: 80vh;
    overflow-y: auto;
    padding: 16px 20px 20px;
    background: var(--panel);
    border-radius: 12px;
    box-shadow: 0 18px 60px rgba(0, 0, 0, 0.28);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 10px;
  }
  h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  .close {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 6px;
    color: var(--text-secondary);
  }
  .close:hover {
    background: var(--hover);
    color: var(--text);
  }
  .close :global(svg) {
    width: 13px;
    height: 13px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  th,
  td {
    padding: 7px 0;
    font-size: 13px;
    text-align: left;
    border-bottom: 1px solid var(--hairline);
  }
  tr:last-child th,
  tr:last-child td {
    border-bottom: none;
  }
  th {
    font-weight: 400;
    color: var(--text);
  }
  td {
    width: 40%;
    text-align: right;
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .about {
    margin: 0 0 10px;
    font-size: 13px;
  }
  .version {
    margin: 4px 0 0;
    font-size: 12px;
    color: var(--text-secondary);
  }
</style>
