// Pure reducer tests for the AG-UI chat panel: wire events fold into
// chat state without a Tauri runtime.
import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { reduceEvent, historyToState, EMPTY_CHAT, type AgentThreadMessage } from "../lib/agent.svelte";

describe("reduceEvent", () => {
  it("accumulates text deltas into one streaming message and closes it on END", () => {
    let s = reduceEvent(EMPTY_CHAT, { type: "RUN_STARTED" });
    expect(s.runActive).toBe(true);
    s = reduceEvent(s, { type: "TEXT_MESSAGE_START", messageId: "m1", role: "assistant" });
    s = reduceEvent(s, { type: "TEXT_MESSAGE_CONTENT", messageId: "m1", delta: "你" });
    s = reduceEvent(s, { type: "TEXT_MESSAGE_CONTENT", messageId: "m1", delta: "好" });
    expect(s.items).toHaveLength(1);
    expect(s.items[0]).toMatchObject({ kind: "assistant", mid: "m1", text: "你好", streaming: true });
    s = reduceEvent(s, { type: "TEXT_MESSAGE_END", messageId: "m1" });
    s = reduceEvent(s, { type: "RUN_FINISHED" });
    expect(s.items[0]!.streaming).toBe(false);
    expect(s.items[0]!.mid).toBe("m1");
    expect(s.runActive).toBe(false);
  });

  it("creates the message when content arrives without a START (mid-run join)", () => {
    const s = reduceEvent(EMPTY_CHAT, { type: "TEXT_MESSAGE_CONTENT", messageId: "m9", delta: "接上" });
    expect(s.items).toHaveLength(1);
    expect(s.items[0]).toMatchObject({ kind: "assistant", mid: "m9", text: "接上", streaming: true });
  });

  it("tracks a tool call from running to done via TOOL_CALL_RESULT", () => {
    let s = reduceEvent(EMPTY_CHAT, { type: "TOOL_CALL_START", toolCallId: "t1", toolCallName: "open_url" });
    s = reduceEvent(s, { type: "TOOL_CALL_ARGS", toolCallId: "t1", delta: "{\"url\":\"example.com\"" });
    s = reduceEvent(s, { type: "TOOL_CALL_ARGS", toolCallId: "t1", delta: "}" });
    // END alone does not settle the status; the result does.
    s = reduceEvent(s, { type: "TOOL_CALL_END", toolCallId: "t1" });
    expect(s.items[0]).toMatchObject({ kind: "tool", toolName: "open_url", toolStatus: "running" });
    expect(s.items[0]!.toolArgs).toBe("{\"url\":\"example.com\"}");
    s = reduceEvent(s, { type: "TOOL_CALL_RESULT", toolCallId: "t1", messageId: "tr1", content: "{}", success: true });
    expect(s.items[0]!.toolStatus).toBe("done");
  });

  it("marks a failed tool result as error", () => {
    let s = reduceEvent(EMPTY_CHAT, { type: "TOOL_CALL_START", toolCallId: "t2", toolCallName: "activate_tab" });
    s = reduceEvent(s, { type: "TOOL_CALL_RESULT", toolCallId: "t2", messageId: "tr2", content: "{\"error\":\"x\"}", success: false });
    expect(s.items[0]!.toolStatus).toBe("error");
  });

  it("caps the streamed args preview", () => {
    let s = reduceEvent(EMPTY_CHAT, { type: "TOOL_CALL_START", toolCallId: "t3", toolCallName: "get_tabs" });
    s = reduceEvent(s, { type: "TOOL_CALL_ARGS", toolCallId: "t3", delta: "x".repeat(400) });
    expect(s.items[0]!.toolArgs.length).toBe(200);
  });

  it("adds an error card and ends the run on RUN_ERROR", () => {
    let s = reduceEvent(EMPTY_CHAT, { type: "RUN_STARTED" });
    s = reduceEvent(s, { type: "RUN_ERROR", message: "連線失敗" });
    expect(s.items.at(-1)).toMatchObject({ kind: "error", text: "連線失敗" });
    expect(s.runActive).toBe(false);
  });

  it("keeps two assistant messages in one run separate", () => {
    let s = reduceEvent(EMPTY_CHAT, { type: "RUN_STARTED" });
    s = reduceEvent(s, { type: "TEXT_MESSAGE_START", messageId: "a", role: "assistant" });
    s = reduceEvent(s, { type: "TEXT_MESSAGE_END", messageId: "a" });
    s = reduceEvent(s, { type: "TEXT_MESSAGE_START", messageId: "b", role: "assistant" });
    s = reduceEvent(s, { type: "TEXT_MESSAGE_CONTENT", messageId: "b", delta: "第二段" });
    expect(s.items.filter((i) => i.kind === "assistant")).toHaveLength(2);
  });

  it("ignores events it does not consume", () => {
    const s = reduceEvent(EMPTY_CHAT, { type: "STATE_SNAPSHOT", snapshot: {} });
    expect(s).toBe(EMPTY_CHAT);
  });

  it("keeps two turns apart when the server reuses wire message ids", () => {
    // The live fixture reuses messageId "m1" every turn; duplicate
    // Svelte keyed-each ids broke rendering until ids became
    // panel-unique with the wire id kept on `mid`.
    const turn = () => [
      { type: "TEXT_MESSAGE_START", messageId: "m1", role: "assistant" },
      { type: "TEXT_MESSAGE_CONTENT", messageId: "m1", delta: "turn text" },
      { type: "TEXT_MESSAGE_END", messageId: "m1" },
      { type: "RUN_FINISHED" },
    ];
    let s = reduceEvent(EMPTY_CHAT, { type: "RUN_STARTED" });
    for (const ev of turn()) s = reduceEvent(s, ev);
    for (const ev of turn()) s = reduceEvent(s, ev);
    const texts = s.items.filter((i) => i.kind === "assistant").map((i) => i.text);
    expect(texts).toEqual(["turn text", "turn text"]);
    const ids = s.items.map((i) => i.id);
    expect(new Set(ids).size, "panel item ids stay unique").toBe(ids.length);
    expect(s.runActive).toBe(false);
  });

  it("keeps two tool calls apart when the server reuses tool call ids", () => {
    const toolTurn = () => [
      { type: "TOOL_CALL_START", toolCallId: "t1", toolCallName: "activate_tab" },
      { type: "TOOL_CALL_ARGS", toolCallId: "t1", delta: '{"tabId":1}' },
      { type: "TOOL_CALL_END", toolCallId: "t1" },
      { type: "TOOL_CALL_RESULT", toolCallId: "t1", messageId: "r", content: "{}", success: true },
    ];
    let s = EMPTY_CHAT;
    for (const ev of toolTurn()) s = reduceEvent(s, ev);
    for (const ev of toolTurn()) s = reduceEvent(s, ev);
    const tools = s.items.filter((i) => i.kind === "tool");
    expect(tools).toHaveLength(2);
    expect(tools.every((t) => t.toolStatus === "done")).toBe(true);
    expect(new Set(s.items.map((i) => i.id)).size).toBe(s.items.length);
  });
});

describe("historyToState", () => {
  it("replays user, assistant text, and tool calls; skips tool results", () => {
    const messages: AgentThreadMessage[] = [
      { id: "u-1", role: "user", content: "切到第二個分頁" },
      { id: "a-1", role: "assistant", content: "", toolCalls: [{ id: "t1", name: "activate_tab" }] },
      { id: "tr-1", role: "tool", content: "{\"ok\":true}", toolCallId: "t1" },
      { id: "a-2", role: "assistant", content: "已切換。" },
    ];
    const s = historyToState(messages);
    expect(s.items.map((i) => [i.kind, i.text || i.toolName])).toEqual([
      ["user", "切到第二個分頁"],
      ["tool", "activate_tab"],
      ["assistant", "已切換。"],
    ]);
    expect(s.runActive).toBe(false);
  });

  it("renders an empty state for an empty thread", () => {
    expect(historyToState([])).toEqual(EMPTY_CHAT);
  });
});
