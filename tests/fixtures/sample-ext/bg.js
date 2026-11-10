// Runs in the extension's QuickJS thread. Counts page loads seen via the
// tabs API and stores a session counter through the storage API.
(async () => {
  try {
    const hits = await surf.callApi("storage", "get", { key: "hits" });
    const n = hits && typeof hits.value === "string" ? Number(hits.value) : 0;
    await surf.callApi("storage", "set", { key: "hits", value: String(n + 1) });
    console.log("bg ready, hits =", n + 1);
  } catch (e) {
    console.error("bg api failed:", String(e));
  }
})();
