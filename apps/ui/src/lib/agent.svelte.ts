// AG-UI chat panel wiring: IPC to the Rust run loop (agui.rs) plus a
// pure event reducer the panel renders from. All agent traffic rides
// `agui://event`; the panel is the only consumer.
import { invoke } from "@tauri-apps/api/core";

export function agentChatSetOpen(open: boolean) {
  void invoke("agent_chat_set_open", { open }).catch((e) => console.error(String(e)));
}

export function agentChatSend(text: string) {
  return invoke<void>("agent_chat_send", { text });
}

export function agentChatHistory() {
  return invoke<AgentThreadMessage[]>("agent_chat_history");
}

export function agentChatReset() {
  return invoke<void>("agent_chat_reset");
}

export interface AgentAguiConfig {
  endpoint: string;
  hasKey: boolean;
}

export function agentAguiConfigGet() {
  return invoke<AgentAguiConfig>("agent_agui_config_get");
}

export function agentAguiConfigSet(endpoint: string, apiKey: string | null, clearKey: boolean) {
  return invoke<void>("agent_agui_config_set", {
    endpoint,
    apiKey: apiKey ?? null,
    clearKey,
  });
}

export interface AgentLlmConfig {
  baseUrl: string;
  model: string;
  hasKey: boolean;
}

export function agentLlmConfigGet() {
  return invoke<AgentLlmConfig>("agent_llm_config_get");
}

export function agentLlmConfigSet(baseUrl: string, model: string, apiKey: string | null, clearKey: boolean) {
  return invoke<void>("agent_llm_config_set", {
    baseUrl,
    model,
    apiKey: apiKey ?? null,
    clearKey,
  });
}

// ---------------------------------------------------------------------------
// Event reducer (pure, unit-tested)

/** One AG-UI wire event, loosely typed: `type` drives everything. */
export interface AguiEvent {
  type: string;
  [key: string]: unknown;
}

export type ToolStatus = "running" | "done" | "error";

export interface ChatItem {
  kind: "user" | "assistant" | "tool" | "error";
  /** Panel-unique id (Svelte keyed-each key). Wire ids can repeat
   * across turns — naive servers reuse message ids — so the panel
   * suffixes them; `mid` holds the wire id for event matching. */
  id: string;
  mid: string;
  /** user / assistant accumulated text, or the error message. */
  text: string;
  /** assistant message still receiving deltas. */
  streaming: boolean;
  /** tool items only. */
  toolName: string;
  toolArgs: string;
  toolStatus: ToolStatus;
}

export interface ChatState {
  items: ChatItem[];
  runActive: boolean;
}

export const EMPTY_CHAT: ChatState = { items: [], runActive: false };

function item(partial: Partial<ChatItem> & Pick<ChatItem, "kind" | "id">): ChatItem {
  return { mid: "", text: "", streaming: false, toolName: "", toolArgs: "", toolStatus: "done", ...partial };
}

/** Panel-unique id from a wire id that may repeat across turns. */
function uniqueId(state: ChatState, wire: string): string {
  return `${wire}#${state.items.length}`;
}

function str(ev: AguiEvent, key: string): string {
  const v = ev[key];
  return typeof v === "string" ? v : "";
}

/** How much of streamed tool args to keep as the chip's preview. */
const ARGS_PREVIEW_LIMIT = 200;

/** Last index matching a predicate (ES2023 findLastIndex is not in
 * the TS lib target yet). */
function findLastIndex<T>(items: T[], pred: (t: T) => boolean): number {
  for (let i = items.length - 1; i >= 0; i--) {
    if (pred(items[i]!)) return i;
  }
  return -1;
}

/** Fold one AG-UI event into the panel state. Pure. */
export function reduceEvent(state: ChatState, ev: AguiEvent): ChatState {
  switch (ev.type) {
    case "RUN_STARTED":
      return { ...state, runActive: true };
    case "RUN_FINISHED":
      return { ...state, runActive: false };
    case "RUN_ERROR":
      return {
        items: [...state.items, item({ kind: "error", id: `err-${state.items.length}`, text: str(ev, "message") || "執行錯誤" })],
        runActive: false,
      };
    case "TEXT_MESSAGE_START": {
      const mid = str(ev, "messageId");
      return {
        ...state,
        items: [...state.items, item({ kind: "assistant", id: uniqueId(state, mid), mid, streaming: true })],
      };
    }
    case "TEXT_MESSAGE_CONTENT": {
      const mid = str(ev, "messageId");
      // A stream joined mid-run (panel reopened): create the message
      // instead of dropping the delta.
      const items = state.items.some((i) => i.kind === "assistant" && i.mid === mid)
        ? state.items
        : [...state.items, item({ kind: "assistant", id: uniqueId(state, mid), mid, streaming: true })];
      // Only the newest stream for this wire id accumulates.
      const lastIdx = findLastIndex(items, (i) => i.kind === "assistant" && i.mid === mid);
      return {
        ...state,
        items: items.map((i, idx) => (idx === lastIdx ? { ...i, text: i.text + str(ev, "delta") } : i)),
      };
    }
    case "TEXT_MESSAGE_END": {
      const mid = str(ev, "messageId");
      // Close the newest open stream for this wire id.
      const lastIdx = findLastIndex(state.items, (i) => i.kind === "assistant" && i.mid === mid && i.streaming);
      if (lastIdx < 0) return state;
      return {
        ...state,
        items: state.items.map((i, idx) => (idx === lastIdx ? { ...i, streaming: false } : i)),
      };
    }
    case "TOOL_CALL_START": {
      const mid = str(ev, "toolCallId");
      return {
        ...state,
        items: [
          ...state.items,
          item({ kind: "tool", id: uniqueId(state, mid), mid, toolName: str(ev, "toolCallName"), toolStatus: "running" }),
        ],
      };
    }
    case "TOOL_CALL_ARGS": {
      const mid = str(ev, "toolCallId");
      const lastIdx = findLastIndex(state.items, (i) => i.kind === "tool" && i.mid === mid);
      if (lastIdx < 0) return state;
      return {
        ...state,
        items: state.items.map((i, idx) =>
          idx === lastIdx ? { ...i, toolArgs: (i.toolArgs + str(ev, "delta")).slice(0, ARGS_PREVIEW_LIMIT) } : i,
        ),
      };
    }
    case "TOOL_CALL_END":
      // Status stays "running" until TOOL_CALL_RESULT lands.
      return state;
    case "TOOL_CALL_RESULT": {
      const mid = str(ev, "toolCallId");
      const failed = ev.success === false;
      const lastIdx = findLastIndex(state.items, (i) => i.kind === "tool" && i.mid === mid);
      if (lastIdx < 0) return state;
      return {
        ...state,
        items: state.items.map((i, idx) => (idx === lastIdx ? { ...i, toolStatus: failed ? "error" : "done" } : i)),
      };
    }
    default:
      return state;
  }
}

/** A thread message as Rust stores it (RunAgentInput shape). */
export interface AgentThreadMessage {
  id: string;
  role: "system" | "user" | "assistant" | "tool";
  content: string;
  toolCalls?: { id: string; name: string }[];
  toolCallId?: string;
}

/** Replay the stored thread for panel-open rendering. Pure. */
export function historyToState(messages: AgentThreadMessage[]): ChatState {
  const items: ChatItem[] = [];
  for (const m of messages) {
    if (m.role === "user") {
      items.push(item({ kind: "user", id: m.id, text: m.content }));
    } else if (m.role === "assistant") {
      const call = m.toolCalls?.[0];
      if (call) {
        // Past tool calls replay as completed; the result message that
        // follows them carries the outcome, not shown in the chip.
        items.push(item({ kind: "tool", id: `${call.id}#${items.length}`, mid: call.id, toolName: call.name, toolStatus: "done" }));
      } else if (m.content) {
        items.push(item({ kind: "assistant", id: m.id, text: m.content }));
      }
    }
    // role "tool" results and any "system" messages have no chip.
  }
  return { items, runActive: false };
}
