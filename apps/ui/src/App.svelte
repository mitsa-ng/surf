<script lang="ts">
  import { onMount } from "svelte";
  import { init, openTab, closeTab, tabs } from "./lib/tabs.svelte";
  import { isMobileDevice } from "./lib/platform";
  import TabStrip from "./lib/components/TabStrip.svelte";
  import Toolbar from "./lib/components/Toolbar.svelte";
  import BottomBar from "./lib/components/BottomBar.svelte";
  import TabSheet from "./lib/components/TabSheet.svelte";

  const isMobile = isMobileDevice();
  let toolbar: Toolbar | undefined = $state();

  onMount(() => {
    void init();
  });

  function onKeydown(e: KeyboardEvent) {
    const meta = e.metaKey || e.ctrlKey;
    if (!meta) return;
    if (e.key === "l") {
      e.preventDefault();
      toolbar?.focusAddressBar();
    } else if (e.key === "t") {
      e.preventDefault();
      void openTab();
    } else if (e.key === "w" && tabs.activeId !== null) {
      e.preventDefault();
      void closeTab(tabs.activeId);
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if isMobile}
  <main class="mobile">
    {#if tabs.sheetOpen}
      <TabSheet />
    {/if}
    <div class="content" aria-hidden="true">
      {#if tabs.list.length === 0}
        <p>沒有分頁。點下方分頁按鈕新增。</p>
      {:else}
        <p>行動端內容分頁在 M2 啟用，先以桌面版瀏覽。</p>
      {/if}
    </div>
    <BottomBar />
  </main>
{:else}
  <header>
    <TabStrip />
    <Toolbar bind:this={toolbar} />
  </header>
  <!-- Content webviews are native children positioned below this chrome;
       nothing renders here. -->
{/if}

<style>
  header {
    position: fixed;
    inset: 0 0 auto 0;
    height: 80px;
    background: var(--panel);
  }
  .mobile {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  .content {
    flex: 1;
    display: grid;
    place-items: center;
    padding: 16px;
    background: var(--bg);
  }
  .content p {
    margin: 0;
    color: var(--text-secondary);
    font-size: 14px;
    text-align: center;
  }
</style>
