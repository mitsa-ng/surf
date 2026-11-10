use std::collections::HashMap;
use std::sync::Arc;

use surf_core::tabs::TabId;

use crate::ExtensionId;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Endpoint {
    Extension(ExtensionId),
    Content(TabId),
    Popup(ExtensionId),
    /// The browser host itself (core services).
    Host,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    pub from: Endpoint,
    pub to: Endpoint,
    pub body: MessageBody,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageBody {
    Event { name: String, payload: serde_json::Value },
    ApiCall { module: String, method: String, payload: serde_json::Value },
    Response { ok: serde_json::Value },
    Error { message: String },
}

#[derive(Debug, thiserror::Error)]
pub enum RouterError {
    #[error("no sink registered at {0:?}")]
    NoRoute(Endpoint),
    #[error("sink rejected: {0}")]
    Rejected(String),
}

pub trait MessageSink: Send + Sync {
    fn deliver(&self, envelope: Envelope) -> Result<(), RouterError>;
}

/// ext ↔ content ↔ popup routing. Sinks register per endpoint; the host
/// bridges sinks to runtimes and content-script bridges.
#[derive(Default)]
pub struct Router {
    sinks: HashMap<Endpoint, Vec<Arc<dyn MessageSink>>>,
}

impl Router {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe(&mut self, endpoint: Endpoint, sink: Arc<dyn MessageSink>) {
        self.sinks.entry(endpoint).or_default().push(sink);
    }

    pub fn unsubscribe_all(&mut self, endpoint: &Endpoint) {
        self.sinks.remove(endpoint);
    }

    pub fn route_count(&self, endpoint: &Endpoint) -> usize {
        self.sinks.get(endpoint).map(Vec::len).unwrap_or(0)
    }

    pub fn send(&self, envelope: Envelope) -> Result<(), RouterError> {
        let sinks = self.sinks.get(&envelope.to).ok_or_else(|| RouterError::NoRoute(envelope.to.clone()))?;
        for sink in sinks {
            sink.deliver(envelope.clone())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct Capture(Arc<Mutex<Vec<Envelope>>>);

    impl MessageSink for Capture {
        fn deliver(&self, envelope: Envelope) -> Result<(), RouterError> {
            self.0.lock().unwrap().push(envelope);
            Ok(())
        }
    }

    fn sample(from: Endpoint, to: Endpoint) -> Envelope {
        Envelope {
            from,
            to,
            body: MessageBody::Event { name: "tabs.created".into(), payload: serde_json::json!({ "id": 1 }) },
        }
    }

    #[test]
    fn delivers_to_subscribed_endpoint() {
        let mut router = Router::new();
        let ext = ExtensionId("e".into());
        let captured = Arc::new(Mutex::new(Vec::new()));
        router.subscribe(Endpoint::Host, Arc::new(Capture(captured.clone())));

        let env = sample(Endpoint::Extension(ext.clone()), Endpoint::Host);
        router.send(env.clone()).unwrap();
        let got = captured.lock().unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0], env);
    }

    #[test]
    fn unrouted_endpoint_errors() {
        let router = Router::new();
        let err = router.send(sample(Endpoint::Host, Endpoint::Content(TabId(9))));
        assert!(matches!(err, Err(RouterError::NoRoute(_))));
    }

    #[test]
    fn unsubscribe_removes_route() {
        let mut router = Router::new();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let ep = Endpoint::Popup(ExtensionId("e".into()));
        router.subscribe(ep.clone(), Arc::new(Capture(captured.clone())));
        assert_eq!(router.route_count(&ep), 1);
        router.unsubscribe_all(&ep);
        assert_eq!(router.route_count(&ep), 0);
        assert!(router.send(sample(Endpoint::Host, ep)).is_err());
    }
}
