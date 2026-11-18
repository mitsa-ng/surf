<script lang="ts">
  import { onMount } from "svelte";
  import {
    agentChatSetOpen,
    agentChatSend,
    agentChatHistory,
    agentChatReset,
    agentAguiConfigGet,
    agentLlmConfigGet,
    reduceEvent,
    historyToState,
    type ChatState,
    type AguiEvent,
  } from "../agent.svelte";
  import Icon from "./Icon.svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";

  // One listener per panel mount: events fold into chat state, unmount
  // stops listening and aborts any in-flight Rust run.
  let chat = $state<ChatState>({ items: [], runActive: false });
  let draft = $state("");
  let sourceLabel = $state("");

  // Which source answers, mirroring Rust precedence: AG-UI endpoint >
  // LLM > 本地示範.
  async function loadSource() {
    try {
      const agui = await agentAguiConfigGet();
      if (agui.endpoint) {
        const host = new URL(agui.endpoint).host;
        sourceLabel = `AG-UI · ${host}`;
        return;
      }
    } catch {
      // Fall through to the next source.
    }
    try {
      const llm = await agentLlmConfigGet();
      if (llm.baseUrl && llm.model && llm.hasKey) {
        sourceLabel = `LLM · ${llm.model}`;
        return;
      }
    } catch {
      // Demo it is.
    }
    sourceLabel = "本地示範模式";
  }

  onMount(() => {
    let unlisten: UnlistenFn | undefined;
    let stopped = false;
    void (async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        unlisten = await listen("agui://event", (event) => {
          chat = reduceEvent(chat, event.payload as AguiEvent);
        });
        if (stopped) return;
        chat = historyToState(await agentChatHistory());
        agentChatSetOpen(true);
        void loadSource();
      } catch (e) {
        console.error(String(e));
      }
    })();
    return () => {
      stopped = true;
      unlisten?.();
      agentChatSetOpen(false);
    };
  });

  function send() {
    const text = draft.trim();
    if (!text || chat.runActive) return;
    draft = "";
    // The user bubble renders locally; the thread on the Rust side is
    // the source of truth and replays it on the next panel open.
    chat = {
      ...chat,
      items: [
        ...chat.items,
        { kind: "user", id: `local-${chat.items.length}`, mid: "", text, streaming: false, toolName: "", toolArgs: "", toolStatus: "done" },
      ],
    };
    void agentChatSend(text).catch((e) => console.error(String(e)));
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      send();
    }
  }

  function reset() {
    void agentChatReset().catch((e) => console.error(String(e)));
    chat = { items: [], runActive: false };
  }

  // New content keeps the newest line visible; reading the last item's
  // text length tracks streaming deltas too.
  let scroller: HTMLElement | undefined = $state();
  $effect(() => {
    const n = chat.items.length;
    const last = chat.items[n - 1];
    void last?.kind;
    void last?.text.length;
    void last?.toolStatus;
    // jsdom has no Element.scrollTo; the optional call keeps tests real.
    scroller?.scrollTo?.({ top: scroller.scrollHeight });
  });

  const statusText = { running: "執行中", done: "完成", error: "錯誤" } as const;
</script>

<section class="agent-panel" aria-label="Agent 面板">
  <header class="head">
    <span class="source" role="status">{sourceLabel}</span>
    <button class="reset" onclick={reset} disabled={chat.items.length === 0}>清除對話</button>
  </header>
  <div class="chat" bind:this={scroller}>
    {#if chat.items.length === 0}
      <p class="greet">我是 Surf 助手。問我關於分頁的問題，或叫我開網址、切換分頁。</p>
    {/if}
    {#each chat.items as it (it.id)}
      {#if it.kind === "user"}
        <div class="msg user">
          <p>{it.text}</p>
        </div>
      {:else if it.kind === "assistant"}
        <div class="msg assistant">
          <p>{it.text}</p>
          {#if it.streaming && !it.text}<span class="dots" aria-label="輸入中">…</span>{/if}
        </div>
      {:else if it.kind === "tool"}
        <div class="msg tool" data-status={it.toolStatus}>
          <span class="tool-name">{it.toolName}</span>
          {#if it.toolArgs}<span class="tool-args">{it.toolArgs}</span>{/if}
          <span class="tool-status">{statusText[it.toolStatus]}</span>
        </div>
      {:else}
        <div class="msg error" role="alert">
          <p>{it.text}</p>
        </div>
      {/if}
    {/each}
    {#if chat.runActive && !chat.items.some((i) => i.streaming)}
      <p class="status" role="status">思考中…</p>
    {/if}
  </div>
  <form
    class="composer"
    onsubmit={(e) => {
      e.preventDefault();
      send();
    }}
  >
    <input
      bind:value={draft}
      onkeydown={onKeydown}
      placeholder="問 agent…"
      aria-label="傳訊息給 agent"
      autocomplete="off"
      spellcheck="false"
    />
    <button type="submit" disabled={!draft.trim() || chat.runActive} aria-label="送出">
      <Icon name="send" />
    </button>
  </form>
</section>

<style>
  .agent-panel {
    display: flex;
    flex-direction: column;
    /* The drawer stacks a 28px close-button header above the panel;
       100% alone overflows the window and pushes the composer off
       screen (live-verified: input top landed at the window edge). */
    height: calc(100% - 28px);
    min-height: 0;
    font-size: 13px;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding-bottom: 8px;
  }
  .source {
    color: var(--text-secondary);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .reset {
    flex: none;
    height: 24px;
    padding: 0 8px;
    border-radius: 6px;
    color: var(--text-secondary);
    font-size: 11px;
  }
  .reset:hover:not(:disabled) {
    background: var(--hover);
    color: var(--text);
  }
  .reset:disabled {
    opacity: 0.4;
  }
  .chat {
    flex: 1;
    min-height: 120px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 4px 0 8px;
  }
  .greet {
    margin: 8px 0;
    color: var(--text-secondary);
    font-size: 12px;
  }
  .msg p {
    margin: 0;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .msg.user {
    align-self: flex-end;
    max-width: 85%;
    padding: 6px 10px;
    border-radius: 12px 12px 3px 12px;
    background: var(--accent);
    color: #fff;
  }
  .msg.assistant {
    align-self: flex-start;
    max-width: 95%;
    padding: 2px 0;
  }
  .msg.assistant .dots {
    color: var(--text-secondary);
  }
  .msg.tool {
    align-self: flex-start;
    display: flex;
    align-items: baseline;
    gap: 8px;
    max-width: 100%;
    padding: 5px 9px;
    border: 1px solid var(--hairline);
    border-radius: 8px;
    font-size: 12px;
  }
  .tool-name {
    flex: none;
    font-family: ui-monospace, monospace;
  }
  .tool-args {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-secondary);
    font-family: ui-monospace, monospace;
    font-size: 11px;
  }
  .tool-status {
    flex: none;
    color: var(--text-secondary);
    font-size: 11px;
  }
  .msg.tool[data-status="error"] .tool-status {
    color: #b3261e;
  }
  .msg.error {
    align-self: stretch;
    padding: 6px 9px;
    border: 1px solid #b3261e;
    border-radius: 8px;
    color: #b3261e;
    font-size: 12px;
  }
  .status {
    margin: 0;
    color: var(--text-secondary);
    font-size: 11px;
  }
  .composer {
    display: flex;
    gap: 8px;
    padding-top: 8px;
    border-top: 1px solid var(--hairline);
  }
  .composer input {
    flex: 1;
    min-width: 0;
    height: 32px;
    padding: 0 10px;
    border: 1px solid var(--hairline);
    border-radius: 16px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
    font-size: 13px;
  }
  .composer button {
    flex: none;
    width: 32px;
    height: 32px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: var(--accent);
    color: #fff;
  }
  .composer button:disabled {
    opacity: 0.4;
  }
  .composer button :global(svg) {
    width: 14px;
    height: 14px;
  }
</style>
