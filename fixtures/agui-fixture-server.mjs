// AG-UI fixture endpoint for live verification (node fixtures/agui-fixture-server.mjs).
//
// A scripted AG-UI agent over the reference HTTP+SSE binding:
//   POST /run  →  text/event-stream of standard events.
//
// Script by conversation shape:
// - A turn whose thread has no tool messages yet answers with streamed
//   text, then proposes one activate_tab call targeting a real tab id
//   from the request's `state` (so the browser provably switches).
// - A turn whose thread carries a tool result answers by quoting it,
//   proving the follow-up run received the client-side execution.
//
// Every request body is pretty-printed to stdout; the VERIFY lines
// assert the loop end-to-end (tab actually activated, tool result
// delivered) without needing screenshots.
import http from "node:http";

const PORT = Number(process.argv[2] ?? 8817);
const CHUNK_DELAY_MS = 70;

function sse(res, event) {
  res.write(`data: ${JSON.stringify(event)}\n\n`);
}

async function stream(res, events) {
  for (const ev of events) {
    await new Promise((r) => setTimeout(r, CHUNK_DELAY_MS));
    sse(res, ev);
  }
}

function lastToolResult(input) {
  const msgs = input.messages ?? [];
  for (let i = msgs.length - 1; i >= 0; i--) {
    if (msgs[i].role === "tool") return msgs[i];
  }
  return null;
}

function pickTargetTab(input) {
  // AG-UI's state is freeform; Surf sends the tab array directly.
  const tabs = Array.isArray(input.state)
    ? input.state
    : Array.isArray(input.state?.tabs)
      ? input.state.tabs
      : [];
  if (tabs.length < 2) return tabs[0] ?? null;
  // Prefer a non-active tab so the switch is observable.
  return tabs.find((t) => !t.active) ?? tabs[0];
}

const server = http.createServer(async (req, res) => {
  if (req.method !== "POST" || !req.url.startsWith("/run")) {
    res.writeHead(404).end();
    return;
  }
  const body = await new Promise((resolve) => {
    let data = "";
    req.on("data", (c) => (data += c));
    req.on("end", () => resolve(data));
  });
  let input;
  try {
    input = JSON.parse(body);
  } catch {
    res.writeHead(400).end("bad json");
    return;
  }
  console.log(`--- run: thread=${input.threadId} messages=${input.messages?.length} ---`);
  console.log(JSON.stringify(input, null, 2));

  res.writeHead(200, {
    "Content-Type": "text/event-stream",
    "Cache-Control": "no-cache",
    Connection: "keep-alive",
  });

  const toolResult = lastToolResult(input);
  if (!toolResult) {
    const target = pickTargetTab(input);
    await stream(res, [
      { type: "RUN_STARTED", threadId: input.threadId, runId: "r1" },
      { type: "TEXT_MESSAGE_START", messageId: "m1", role: "assistant" },
      { type: "TEXT_MESSAGE_CONTENT", messageId: "m1", delta: "收到，我來切換分頁。" },
      { type: "TEXT_MESSAGE_END", messageId: "m1" },
    ]);
    if (!target) {
      await stream(res, [
        { type: "TEXT_MESSAGE_START", messageId: "m2", role: "assistant" },
        { type: "TEXT_MESSAGE_CONTENT", messageId: "m2", delta: "state 裡沒有分頁可切。" },
        { type: "TEXT_MESSAGE_END", messageId: "m2" },
        { type: "RUN_FINISHED" },
      ]);
    } else {
      await stream(res, [
        { type: "TOOL_CALL_START", toolCallId: "t1", toolCallName: "activate_tab" },
        { type: "TOOL_CALL_ARGS", toolCallId: "t1", delta: `{"tabId":${target.id}}` },
        { type: "TOOL_CALL_END", toolCallId: "t1" },
        { type: "RUN_FINISHED" },
      ]);
      console.log(`VERIFY(1): proposed activate_tab for tabId=${target.id}`);
    }
  } else {
    // The follow-up run: prove the tool result arrived and the state
    // now shows the activated tab as active.
    const tabs = Array.isArray(input.state) ? input.state : input.state?.tabs ?? [];
    const active = tabs.find((t) => t.active);
    const okResult = toolResult.content && !toolResult.content.includes('"error"');
    console.log(`VERIFY(2): tool result received: ${toolResult.content}`);
    console.log(`VERIFY(2): active tab is now ${active?.id ?? "none"} (${active?.title ?? ""})`);
    await stream(res, [
      { type: "RUN_STARTED", threadId: input.threadId, runId: "r2" },
      { type: "TEXT_MESSAGE_START", messageId: "m3", role: "assistant" },
      {
        type: "TEXT_MESSAGE_CONTENT",
        messageId: "m3",
        delta: okResult
          ? `已切到分頁 ${active?.id ?? "?"}（${active?.title || active?.url || ""}）。工具結果：${toolResult.content.slice(0, 60)}`
          : `工具執行失敗：${toolResult.content}`,
      },
      { type: "TEXT_MESSAGE_END", messageId: "m3" },
      { type: "RUN_FINISHED" },
    ]);
  }
  res.end();
});

server.listen(PORT, "127.0.0.1", () => {
  console.log(`AG-UI fixture endpoint on http://127.0.0.1:${PORT}/run`);
});
