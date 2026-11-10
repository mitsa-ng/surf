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

            // Minimal `surf` namespace: surf.emit + surf.onEvent registry.
            ctx.eval::<(), _>(
                r#"
                globalThis.surf = {
                  onEvent: null,
                  emit: (name, payload) => surfEmit(name, JSON.stringify(payload === undefined ? null : payload))
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
        Ok(())
    }

    /// Fire `surf.onEvent(name, payload)` inside the script. Returns
    /// false when no handler is registered.
    pub fn dispatch_event(&self, name: &str, payload: &serde_json::Value) -> Result<bool, RuntimeError> {
        let payload = payload.to_string();
        self.budget.set(0);
        let handled = self
            .ctx
            .with(|ctx| -> Result<bool, rquickjs::Error> {
                let surf = ctx.globals().get::<_, rquickjs::Value>("surf")?;
                let Some(surf_obj) = surf.into_object() else {
                    return Ok(false);
                };
                let handler = match surf_obj.get::<_, rquickjs::Function>("onEvent") {
                    Ok(f) if f.is_function() => f,
                    _ => return Ok(false),
                };
                handler.call::<_, rquickjs::Value>((name.to_owned(), payload))?;
                Ok(true)
            })
            .map_err(|e| RuntimeError::Js(e.to_string()))?;
        Ok(handled)
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
    fn no_handler_reports_false() {
        let rt = ExtRuntime::new(ExtensionId("test".into()), RuntimeLimits::default()).unwrap();
        assert!(!rt.dispatch_event("x", &serde_json::json!(null)).unwrap());
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
