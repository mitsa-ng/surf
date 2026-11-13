<script lang="ts">
  import { onMount } from "svelte";
  import { setAgentPanelOpen } from "../agent.svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";

  // One agent turn per panel mount: the MessageProcessor and the event
  // listener live and die with this component, so a surface can never
  // leak from a previous turn.
  let surface = $state<unknown>(undefined);
  let failed = $state(false);

  onMount(() => {
    let unlisten: UnlistenFn | undefined;
    let stopped = false;
    void (async () => {
      try {
        const [{ MessageProcessor }, lit] = await Promise.all([
          import("@a2ui/web_core/v0_9"),
          // Side-effect import: registers <a2ui-surface> and the basic
          // catalog widgets as custom elements.
          import("@a2ui/lit/v0_9"),
        ]);
        if (stopped) return;
        const processor = new MessageProcessor([lit.basicCatalog]);
        processor.onSurfaceCreated((s) => {
          surface = s;
        });
        processor.onSurfaceDeleted(() => {
          surface = undefined;
        });
        const { listen } = await import("@tauri-apps/api/event");
        if (stopped) return;
        unlisten = await listen("agent://message", (event) => {
          processor.processMessages([event.payload] as never);
        });
        setAgentPanelOpen(true);
      } catch (e) {
        console.error(String(e));
        failed = true;
      }
    })();
    return () => {
      stopped = true;
      unlisten?.();
      setAgentPanelOpen(false);
    };
  });

  // The surface is a property on the a2ui web component; assigning it
  // through the element reference avoids framework attribute coercion.
  let host: HTMLElement | undefined = $state();
  $effect(() => {
    const el = host?.querySelector("a2ui-surface") as (HTMLElement & { surface?: unknown }) | null;
    if (el) el.surface = surface;
  });
</script>

<section class="agent-panel" aria-label="Agent 面板">
  <div class="a2ui-host" bind:this={host}>
    {#if failed}
      <p class="status" role="alert">agent 面板無法啟動（元件載入失敗）。</p>
    {:else if !surface}
      <p class="status" role="status">正在連線 agent…</p>
    {/if}
    <a2ui-surface></a2ui-surface>
  </div>
</section>

<style>
  .agent-panel {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .a2ui-host {
    /* DESIGN.md tokens ride through a2ui's CSS variables: the accent is
       the one deliberate color, everything else stays greyscale. Light
       and dark both come from the existing theme variables. */
    --a2ui-primary-color: var(--accent);
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 120px;
    font-size: 13px;
  }
  .status {
    margin: 8px 0;
    color: var(--text-secondary);
    font-size: 12px;
  }
  a2ui-surface {
    display: block;
    color: var(--text);
  }
</style>
