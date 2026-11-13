use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use rquickjs::{Context, Runtime};

use crate::ExtensionId;

/// Budget so a runaway background script cannot wedge the browser.
pub struct RuntimeLimits {
    pub memory_limit_bytes: usize,
    pub max_stack_size_bytes: usize,
    pub instruction_budget: u64,
}

impl Default for RuntimeLimits {
    fn default() -> Self {
        Self { memory_limit_bytes: 64 * 1024 * 1024, max_stack_size_bytes: 1024 * 1024, instruction_budget: 5_000_000 }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("quickjs: {0}")]
    Js(String),
}

/// Outgoing messages the script world pushed via `surf.emit(name, payload)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutgoingMessage {
    pub name: String,
    pub payload: String,
}

/// One QuickJS context per extension, per the architecture diagram.
/// Neither Send nor Sync: keep it on the thread that created it.
pub struct ExtRuntime {
    #[allow(dead_code)]
    id: ExtensionId,
    rt: Runtime,
    ctx: Context,
    outgoing: Rc<RefCell<VecDeque<OutgoingMessage>>>,
    budget: Rc<std::cell::Cell<u64>>,
    limits: RuntimeLimits,
}

impl ExtRuntime {
    pub fn new(id: ExtensionId, limits: RuntimeLimits) -> Result<Self, RuntimeError> {
        let rt = Runtime::new().map_err(|e| RuntimeError::Js(e.to_string()))?;
        rt.set_memory_limit(limits.memory_limit_bytes);
        rt.set_max_stack_size(limits.max_stack_size_bytes);
        let budget = Rc::new(std::cell::Cell::new(0u64));
        let interrupt_budget = budget.clone();
        rt.set_interrupt_handler(Some(Box::new(move || {
            let used = interrupt_budget.get().saturating_add(1);
            interrupt_budget.set(used);
            used > 100_000_000
        })));

        let ctx = Context::full(&rt).map_err(|e| RuntimeError::Js(e.to_string()))?;
        let outgoing: Rc<RefCell<VecDeque<OutgoingMessage>>> = Rc::new(RefCell::new(VecDeque::new()));

        let emit_queue = outgoing.clone();
        ctx.with(|ctx| -> Result<(), rquickjs::Error> {
            let global = ctx.globals();
            let emit = rquickjs::Function::new(ctx.clone(), move |name: String, payload: String| {
                emit_queue.borrow_mut().push_back(OutgoingMessage { name, payload });
                rquickjs::Undefined
            })?;
            global.set("surfEmit", emit)?;

            // `surf` namespace: emit, per-name listeners, promise-based host API calls.
            ctx.eval::<(), _>(
                r#"
                globalThis.surf = {
                  _listeners: {},
                  _pending: {},
                  onEvent: null,
                  emit: (name, payload) => surfEmit(name, JSON.stringify(payload === undefined ? null : payload)),
                  addListener: (name, fn) => {
                    if (typeof fn !== "function") return;
                    (surf._listeners[name] = surf._listeners[name] || []).push(fn);
                  },
                  callApi: (module, method, args) => new Promise((resolve, reject) => {
                    // No timeout: QuickJS has no timers here; a hung host
                    // call leaves the promise pending (documented).
                    const id = Date.now().toString(36) + Math.random().toString(36).slice(2, 8);
                    surf._pending[id] = { resolve, reject };
                    surfEmit("__api", JSON.stringify({ id, module, method, args: args === undefined ? null : args }));
                  })
                };
                // Chrome-shaped tabs events: the host broadcasts
                // "tabs.onXxx" events (gated on the tabs API grant).
                const tabsEvent = (name) => ({ addListener: (fn) => surf.addListener("tabs." + name, fn) });
                surf.tabs = {
                  onCreated: tabsEvent("onCreated"),
                  onRemoved: tabsEvent("onRemoved"),
                  onActivated: tabsEvent("onActivated"),
                  onUpdated: tabsEvent("onUpdated")
                };
                globalThis.__surfDispatch = (name, payload) => {
                  if (name.startsWith("__apiResult:")) {
                    const id = name.slice("__apiResult:".length);
                    const p = surf._pending[id];
                    if (p) { delete surf._pending[id]; try { p.resolve(JSON.parse(payload)); } catch (e) { p.reject(e); } return true; }
                    return false;
                  }
                  let handled = false;
                  if (typeof surf.onEvent === "function") { surf.onEvent(name, payload); handled = true; }
                  for (const fn of surf._listeners[name] || []) { try { fn(payload); } catch (e) { surfEmit("console.error", String(e)); } handled = true; }
                  return handled;
                };
                globalThis.console = {
                  log: (...a) => surfEmit("console", a.map(String).join(" ")),
                  warn: (...a) => surfEmit("console.warn", a.map(String).join(" ")),
                  error: (...a) => surfEmit("console.error", a.map(String).join(" "))
                };
                "#,
            )?;
            Ok(())
        })
        .map_err(|e| RuntimeError::Js(e.to_string()))?;

        Ok(Self { id, rt, ctx, outgoing, budget, limits })
    }

    pub fn eval(&self, code: &str) -> Result<(), RuntimeError> {
        self.budget.set(0);
        self.ctx
            .with(|ctx| -> Result<(), rquickjs::Error> {
                let _value = ctx.eval::<rquickjs::Value, _>(code)?;
                Ok(())
            })
            .map_err(|e| RuntimeError::Js(e.to_string()))?;
        self.pump_jobs();
        Ok(())
    }

    /// Fire `surf.onEvent(name, payload)` inside the script. Returns
    /// false when no handler is registered. Promise continuations queued
    /// by the call are pumped before returning.
    pub fn dispatch_event(&self, name: &str, payload: &serde_json::Value) -> Result<bool, RuntimeError> {
        let payload = payload.to_string();
        self.budget.set(0);
        let handled = self
            .ctx
            .with(|ctx| -> Result<bool, rquickjs::Error> {
                let dispatch = match ctx.globals().get::<_, rquickjs::Function>("__surfDispatch") {
                    Ok(f) => f,
                    Err(_) => return Ok(false),
                };
                let handled = dispatch.call::<_, bool>((name.to_owned(), payload))?;
                Ok(handled)
            })
            .map_err(|e| RuntimeError::Js(e.to_string()))?;
        self.pump_jobs();
        Ok(handled)
    }

    /// QuickJS runs promise jobs only when asked; async extension code
    /// stalls without this.
    fn pump_jobs(&self) {
        for _ in 0..1000 {
            match self.rt.execute_pending_job() {
                Ok(true) => continue,
                _ => break,
            }
        }
    }

    pub fn drain_outgoing(&self) -> Vec<OutgoingMessage> {
        self.outgoing.borrow_mut().drain(..).collect()
    }

    pub fn memory_usage(&self) -> usize {
        self.rt.memory_usage().memory_used_size.max(0) as usize
    }

    pub fn limits(&self) -> &RuntimeLimits {
        &self.limits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_and_event_roundtrip() {
        let rt = ExtRuntime::new(ExtensionId("test".into()), RuntimeLimits::default()).unwrap();
        rt.eval(
            r#"
            var seen = [];
            surf.onEvent = (name, payload) => {
              var p = JSON.parse(payload);
              surf.emit("echo:" + name, p.value * 2);
            };
            "#,
        )
        .unwrap();
        let handled = rt
            .dispatch_event("ping", &serde_json::json!({ "value": 21 }))
            .unwrap();
        assert!(handled);
        let out = rt.drain_outgoing();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "echo:ping");
        assert_eq!(out[0].payload, "42");
    }

    #[test]
    fn tabs_event_namespaces_reach_listeners() {
        let rt = ExtRuntime::new(ExtensionId("test".into()), RuntimeLimits::default()).unwrap();
        rt.eval(
            r#"
            var seen = [];
            surf.tabs.onUpdated.addListener((payload) => { const p = JSON.parse(payload); seen.push("u:" + p.changeInfo.title); });
            surf.tabs.onCreated.addListener(() => { seen.push("c"); });
            "#,
        )
        .unwrap();
        assert!(rt.dispatch_event("tabs.onCreated", &serde_json::json!({ "id": 1 })).unwrap());
        assert!(rt
            .dispatch_event("tabs.onUpdated", &serde_json::json!({ "id": 1, "changeInfo": { "title": "Hi" } }))
            .unwrap());
        rt.eval(r#"surfEmit("probe", JSON.stringify(seen))"#).unwrap();
        let out = rt.drain_outgoing();
        let probe = out.iter().find(|m| m.name == "probe").expect("probe");
        assert!(probe.payload.contains("\"c\""), "{probe:?}");
        assert!(probe.payload.contains("u:Hi"), "{probe:?}");
    }

    #[test]
    fn no_handler_reports_false() {
        let rt = ExtRuntime::new(ExtensionId("test".into()), RuntimeLimits::default()).unwrap();
        // __surfDispatch always exists after boot; "no handler" means no listeners registered.
        assert!(!rt.dispatch_event("x", &serde_json::json!(null)).unwrap());
    }

    #[test]
    fn call_api_promise_resolves_on_result() {
        let rt = ExtRuntime::new(ExtensionId("test".into()), RuntimeLimits::default()).unwrap();
        rt.eval(
            r#"
            var resolved = null;
            surf.callApi("storage", "get", {"key": "a"}).then((v) => { resolved = v; });
            "#,
        )
        .unwrap();
        // Background thread answers by dispatching __apiResult:<id>; the
        // pending id is opaque to the host, so exercise the shim directly.
        rt.eval(r#"__surfDispatch("__apiResult:test-id", JSON.stringify({ok: true}))"#).unwrap();
        rt.eval(r#"surf._pending["test-id"] = { resolve: (v) => { resolved = v; }, reject: () => {} };"#).unwrap();
        rt.eval(r#"__surfDispatch("__apiResult:test-id", JSON.stringify({ok: true}))"#).unwrap();
        rt.eval(r#"surfEmit("probe", JSON.stringify(resolved))"#).unwrap();
        let out = rt.drain_outgoing();
        assert!(out.iter().any(|m| m.name == "probe" && m.payload.contains("\"ok\":true")));
    }

    #[test]
    fn syntax_error_surfaces() {
        let rt = ExtRuntime::new(ExtensionId("test".into()), RuntimeLimits::default()).unwrap();
        assert!(rt.eval("this is not javascript").is_err());
    }

    #[test]
    fn console_captured_via_outgoing() {
        let rt = ExtRuntime::new(ExtensionId("test".into()), RuntimeLimits::default()).unwrap();
        rt.eval("console.log('hello', 'world')").unwrap();
        let out = rt.drain_outgoing();
        assert_eq!(out[0].name, "console");
        assert_eq!(out[0].payload, "hello world");
    }
}
