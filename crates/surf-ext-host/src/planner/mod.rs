use url::Url;

use crate::manifest::RunAt;
use crate::{ExtensionId, InstalledExtension};

#[derive(Debug, thiserror::Error)]
pub enum PatternError {
    #[error("pattern must contain '://'")]
    NoSeparator,
    #[error("unsupported scheme: {0}")]
    Scheme(String),
    #[error("path must start with '/'")]
    Path,
    #[error("empty host")]
    Host,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scheme {
    All,
    Http,
    Https,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Host {
    Any,
    /// `*.example.com`: matches example.com and any subdomain.
    WildcardSuffix(String),
    Exact(String),
}

/// Chrome-style match pattern: `<scheme>://<host><path>` where path
/// segments may contain `*`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchPattern {
    pub scheme: Scheme,
    pub host: Host,
    pub path: String,
}

impl MatchPattern {
    pub fn parse(pattern: &str) -> Result<Self, PatternError> {
        let (scheme, rest) = pattern.split_once("://").ok_or(PatternError::NoSeparator)?;
        let scheme = match scheme {
            "*" => Scheme::All,
            "http" => Scheme::Http,
            "https" => Scheme::Https,
            "file" => Scheme::File,
            other => return Err(PatternError::Scheme(other.to_owned())),
        };
        let (host, path) = match rest.split_once('/') {
            Some((h, p)) => (h, format!("/{p}")),
            None => (rest, "/*".to_owned()),
        };
        if host.is_empty() {
            return Err(PatternError::Host);
        }
        let host = if host == "*" {
            Host::Any
        } else if let Some(suffix) = host.strip_prefix("*.") {
            Host::WildcardSuffix(suffix.to_ascii_lowercase())
        } else {
            Host::Exact(host.to_ascii_lowercase())
        };
        Ok(Self { scheme, host, path })
    }

    pub fn matches(&self, url: &Url) -> bool {
        match self.scheme {
            Scheme::All => {}
            Scheme::Http if url.scheme() == "http" => {}
            Scheme::Https if url.scheme() == "https" => {}
            Scheme::File if url.scheme() == "file" => {}
            _ => return false,
        }
        // Chrome's "*" scheme matches http and https only.
        if self.scheme == Scheme::All && !matches!(url.scheme(), "http" | "https") {
            return false;
        }
        let host = match url.host_str() {
            Some(h) => h.to_ascii_lowercase(),
            None => return false,
        };
        let host_ok = match &self.host {
            Host::Any => true,
            Host::Exact(h) => host == *h,
            Host::WildcardSuffix(suffix) => host == *suffix || host.ends_with(&format!(".{suffix}")),
        };
        if !host_ok {
            return false;
        }
        path_matches(&self.path, url.path())
    }
}

fn path_matches(pattern: &str, path: &str) -> bool {
    // Match the special case "/" exactly, per Chrome semantics.
    if pattern == "/" {
        return path == "/";
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    let mut rest = path;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            match rest.strip_prefix(part) {
                Some(r) => rest = r,
                None => return false,
            }
        } else if i == parts.len() - 1 {
            if !rest.ends_with(part) {
                return false;
            }
        } else {
            match rest.find(part) {
                Some(idx) => rest = &rest[idx + part.len()..],
                None => return false,
            }
        }
    }
    true
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedScript {
    pub ext: ExtensionId,
    pub script: String,
    pub run_at: RunAt,
}

/// Resolves which content scripts to inject for a navigation target.
#[derive(Default)]
pub struct InjectionPlanner;

impl InjectionPlanner {
    pub fn plan(&self, installed: &[&InstalledExtension], url: &Url) -> Vec<PlannedScript> {
        let mut plan = Vec::new();
        for ext in installed {
            for cs in &ext.manifest.content_scripts {
                let matched = cs.matches.iter().filter_map(|p| MatchPattern::parse(p).ok()).any(|p| p.matches(url));
                if matched {
                    for js in &cs.js {
                        plan.push(PlannedScript { ext: ext.id.clone(), script: js.clone(), run_at: cs.run_at });
                    }
                }
            }
        }
        plan
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn parse_shapes() {
        assert_eq!(
            MatchPattern::parse("*://*/*").unwrap(),
            MatchPattern { scheme: Scheme::All, host: Host::Any, path: "/*".into() }
        );
        assert_eq!(
            MatchPattern::parse("https://*.example.com/foo*bar").unwrap(),
            MatchPattern {
                scheme: Scheme::Https,
                host: Host::WildcardSuffix("example.com".into()),
                path: "/foo*bar".into()
            }
        );
        assert!(MatchPattern::parse("ftp://x/").is_err());
        assert!(MatchPattern::parse("https://").is_err());
    }

    #[test]
    fn wildcard_scheme_is_http_https_only() {
        let all = MatchPattern::parse("*://*/*").unwrap();
        assert!(all.matches(&u("http://a.example/x")));
        assert!(all.matches(&u("https://a.example/x")));
        assert!(!all.matches(&u("file:///tmp/x")));
    }

    #[test]
    fn subdomain_matching() {
        let p = MatchPattern::parse("https://*.example.com/*").unwrap();
        assert!(p.matches(&u("https://example.com/")));
        assert!(p.matches(&u("https://a.example.com/")));
        assert!(p.matches(&u("https://deep.a.example.com/")));
        assert!(!p.matches(&u("https://notexample.com/")));
    }

    #[test]
    fn path_semantics() {
        let root = MatchPattern::parse("https://a.example/").unwrap();
        assert!(root.matches(&u("https://a.example/")));
        // "/": exact root only — "/*" is appended only when no path given.
        assert!(!root.matches(&u("https://a.example/sub")));

        let star = MatchPattern::parse("https://a.example/*").unwrap();
        assert!(star.matches(&u("https://a.example/anything/here")));

        let mid = MatchPattern::parse("https://a.example/foo*bar").unwrap();
        assert!(mid.matches(&u("https://a.example/fooXYZbar")));
        // `*` may match the empty string.
        assert!(mid.matches(&u("https://a.example/foobar")));
        assert!(!mid.matches(&u("https://a.example/fooqux")));

        // Query strings are not part of pattern matching.
        assert!(star.matches(&u("https://a.example/page?q=1")));
    }

    #[test]
    fn case_insensitive_host() {
        let p = MatchPattern::parse("https://EXAMPLE.com/*").unwrap();
        assert!(p.matches(&u("https://example.COM/x")));
    }
}
