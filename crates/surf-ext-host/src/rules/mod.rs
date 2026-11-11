pub mod content_blocker;

use url::Url;

/// What a platform's request-filter adapter can actually enforce. Rules
/// the platform cannot run are dropped at plan time instead of failing
/// navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FilterCapabilities {
    pub block: bool,
    pub redirect: bool,
    pub modify_headers: bool,
}

impl FilterCapabilities {
    /// WKContentRuleList is declarative block-only: no redirects, no
    /// header rewrites.
    pub const APPLE: Self = Self { block: true, redirect: false, modify_headers: false };
    /// WebView2 WebResourceRequested can inspect and replace responses.
    pub const WEBVIEW2: Self = Self { block: true, redirect: true, modify_headers: true };
    /// shouldInterceptRequest can block and substitute responses.
    pub const ANDROID: Self = Self { block: true, redirect: true, modify_headers: false };
    /// WebKitGTK (WebKitUserContentFilter) is block-only like Apple.
    pub const WEBKITGTK: Self = Self { block: true, redirect: false, modify_headers: false };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestAction {
    Allow,
    Block,
    Redirect(Url),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum RuleAction {
    Block,
    Redirect { to: String },
    #[serde(rename_all = "camelCase")]
    ModifyHeaders { set: Vec<HeaderOp>, remove: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderOp {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeclarativeRule {
    pub id: String,
    pub priority: u32,
    pub condition: RuleCondition,
    pub action: RuleAction,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleCondition {
    /// Substring matched against the request URL.
    pub url_filter: String,
}

pub trait RulesEngine {
    fn decide(&self, url: &Url, source: Option<&Url>) -> RequestAction;
}

/// adblock-rust network-filter engine (EasyList syntax).
pub struct AdblockEngine {
    engine: adblock::Engine,
}

#[derive(Debug, thiserror::Error)]
pub enum RuleParseError {
    #[error("adblock rejected filter: {0}")]
    Adblock(String),
    #[error("redirect target is not a valid url: {0}")]
    Redirect(String),
}

impl AdblockEngine {
    pub fn empty() -> Self {
        Self { engine: adblock::Engine::new_with_filter_set(adblock::FilterSet::default()) }
    }

    pub fn from_filters<'a>(filters: impl IntoIterator<Item = &'a str>) -> Result<Self, RuleParseError> {
        let list_text = filters.into_iter().collect::<Vec<_>>().join("\n");
        let mut set = adblock::FilterSet::default();
        // Unparsable lines are skipped by the crate rather than failing the set.
        set.add_filter_list(list_text, adblock::lists::ParseOptions::default());
        Ok(Self { engine: adblock::Engine::new_with_filter_set(set) })
    }
}

impl RulesEngine for AdblockEngine {
    fn decide(&self, url: &Url, source: Option<&Url>) -> RequestAction {
        let source = source.map(|s| s.as_str()).unwrap_or("");
        let request = match adblock::request::Request::new(url.as_str(), source, "other", "get") {
            Ok(r) => r,
            Err(_) => return RequestAction::Allow,
        };
        if self.engine.check_network_request(&request).should_block() {
            RequestAction::Block
        } else {
            RequestAction::Allow
        }
    }
}

/// Surf declarative rules (MV3-inspired). Highest priority wins; ties
/// keep insertion order.
pub struct DeclarativeEngine {
    rules: Vec<DeclarativeRule>,
    capabilities: FilterCapabilities,
}

impl DeclarativeEngine {
    pub fn new(capabilities: FilterCapabilities) -> Self {
        Self { rules: Vec::new(), capabilities }
    }

    pub fn add(&mut self, rule: DeclarativeRule) {
        self.rules.push(rule);
        self.rules.sort_by_key(|r| std::cmp::Reverse(r.priority));
    }

    fn effective_action(&self, rule: &DeclarativeRule) -> Option<RequestAction> {
        match &rule.action {
            RuleAction::Block if self.capabilities.block => Some(RequestAction::Block),
            RuleAction::Redirect { to } if self.capabilities.redirect => {
                Url::parse(to).ok().map(RequestAction::Redirect)
            }
            // Header rewrites and platform-unsupported actions degrade to allow.
            RuleAction::ModifyHeaders { .. } => None,
            _ => None,
        }
    }
}

impl RulesEngine for DeclarativeEngine {
    fn decide(&self, url: &Url, _source: Option<&Url>) -> RequestAction {
        for rule in &self.rules {
            if !url.as_str().contains(&rule.condition.url_filter) {
                continue;
            }
            if let Some(action) = self.effective_action(rule) {
                return action;
            }
        }
        RequestAction::Allow
    }
}

/// First match wins across engines. Not Send/Sync: adblock's Engine is
/// Rc-based, so the chain lives on the thread that built it (the shell
/// wraps it in a dedicated-thread or Mutex arrangement).
pub struct RulesChain {
    engines: Vec<Box<dyn RulesEngine>>,
}

impl RulesChain {
    pub fn new(engines: Vec<Box<dyn RulesEngine>>) -> Self {
        Self { engines }
    }
}

impl RulesEngine for RulesChain {
    fn decide(&self, url: &Url, source: Option<&Url>) -> RequestAction {
        for engine in &self.engines {
            match engine.decide(url, source) {
                RequestAction::Allow => continue,
                action => return action,
            }
        }
        RequestAction::Allow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adblock_blocks_matching_filters() {
        let engine = AdblockEngine::from_filters(["||ads.example^"]).unwrap();
        let blocked = Url::parse("https://ads.example/pixel.png").unwrap();
        let clean = Url::parse("https://example.com/pixel.png").unwrap();
        assert_eq!(engine.decide(&blocked, None), RequestAction::Block);
        assert_eq!(engine.decide(&clean, None), RequestAction::Allow);
    }

    #[test]
    fn declarative_respects_capabilities() {
        let mut engine = DeclarativeEngine::new(FilterCapabilities::APPLE);
        engine.add(DeclarativeRule {
            id: "r1".into(),
            priority: 1,
            condition: RuleCondition { url_filter: "tracker".into() },
            action: RuleAction::Redirect { to: "https://surf.invalid/blocked".into() },
        });
        let url = Url::parse("https://x.example/tracker.js").unwrap();
        // Apple cannot redirect: rule degrades to allow.
        assert_eq!(engine.decide(&url, None), RequestAction::Allow);

        let mut engine = DeclarativeEngine::new(FilterCapabilities::WEBVIEW2);
        engine.add(DeclarativeRule {
            id: "r1".into(),
            priority: 1,
            condition: RuleCondition { url_filter: "tracker".into() },
            action: RuleAction::Redirect { to: "https://surf.invalid/blocked".into() },
        });
        assert_eq!(
            engine.decide(&url, None),
            RequestAction::Redirect(Url::parse("https://surf.invalid/blocked").unwrap())
        );
    }

    #[test]
    fn chain_first_match_wins() {
        let mut declarative = DeclarativeEngine::new(FilterCapabilities::WEBVIEW2);
        declarative.add(DeclarativeRule {
            id: "block".into(),
            priority: 10,
            condition: RuleCondition { url_filter: "ads.example".into() },
            action: RuleAction::Block,
        });
        let chain = RulesChain::new(vec![
            Box::new(AdblockEngine::from_filters(["||slowads.example^"]).unwrap()),
            Box::new(declarative),
        ]);
        assert_eq!(
            chain.decide(&Url::parse("https://ads.example/x").unwrap(), None),
            RequestAction::Block
        );
        assert_eq!(
            chain.decide(&Url::parse("https://slowads.example/x").unwrap(), None),
            RequestAction::Block
        );
        assert_eq!(
            chain.decide(&Url::parse("https://fine.example/x").unwrap(), None),
            RequestAction::Allow
        );
    }
}
