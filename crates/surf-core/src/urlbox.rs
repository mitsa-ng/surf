use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OmniboxResult {
    Navigate(Url),
    Search(String),
}

/// Decide whether omnibox input is a URL to load or a search query.
///
/// Follows the usual desktop-browser heuristics: explicit web schemes
/// navigate; bare host-like strings upgrade to https (http for
/// loopback/private hosts, where TLS is rarely deployed); anything with
/// spaces or an unrecognised scheme becomes a search term.
pub fn resolve(input: &str) -> OmniboxResult {
    let s = input.trim();
    if s.is_empty() {
        return OmniboxResult::Search(String::new());
    }
    if s.chars().any(char::is_whitespace) {
        return OmniboxResult::Search(s.to_owned());
    }
    if let Ok(url) = Url::parse(s) {
        match url.scheme() {
            "http" | "https" | "file" | "surf" | "ext" => return OmniboxResult::Navigate(url),
            _ => {}
        }
    }
    if let Ok(url) = Url::parse(&format!("https://{s}")) {
        if let Some(host) = url.host_str() {
            if is_loopback(host) {
                // Url::set_scheme fails when ports change meaning; rebuild instead.
                if let Ok(plain) = Url::parse(&format!("http://{s}")) {
                    return OmniboxResult::Navigate(plain);
                }
            } else if host.contains('.') && host.chars().next().unwrap_or(' ') != '.' {
                return OmniboxResult::Navigate(url);
            }
        }
    }
    OmniboxResult::Search(s.to_owned())
}

fn is_loopback(host: &str) -> bool {
    host == "localhost" || host == "[::1]" || host.starts_with("127.")
}

/// Known search engines; `custom` carries a user template with a `{q}`
/// placeholder (e.g. "https://kagi.com/search?q={q}").
pub fn search_url(engine: &str, custom_template: Option<&str>, query: &str) -> OmniboxResult {
    let encoded = url_encode(query);
    let built = match (engine, custom_template) {
        ("google", _) => Url::parse_with_params("https://www.google.com/search", &[("q", query)])
            .expect("static base parses"),
        ("bing", _) => Url::parse_with_params("https://www.bing.com/search", &[("q", query)])
            .expect("static base parses"),
        ("duckduckgo", _) => Url::parse_with_params("https://duckduckgo.com/", &[("q", query)])
            .expect("static base parses"),
        (_, Some(template)) => {
            if !template.contains("{q}") || template.contains(' ') {
                return OmniboxResult::Search(query.to_owned());
            }
            match Url::parse(&template.replacen("{q}", &encoded, 1)) {
                Ok(u) => u,
                Err(_) => return OmniboxResult::Search(query.to_owned()),
            }
        }
        (_, None) => Url::parse_with_params("https://duckduckgo.com/", &[("q", query)])
            .expect("static base parses"),
    };
    OmniboxResult::Navigate(built)
}

/// Percent-encode everything except unreserved characters; the template is
/// spliced into a string, so the query cannot smuggle extra parameters.
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nav(s: &str) -> OmniboxResult {
        resolve(s)
    }

    #[test]
    fn explicit_schemes_navigate() {
        assert_eq!(
            nav("https://example.com/"),
            OmniboxResult::Navigate(Url::parse("https://example.com/").unwrap())
        );
        assert_eq!(
            nav("http://a.b/c?q=1"),
            OmniboxResult::Navigate(Url::parse("http://a.b/c?q=1").unwrap())
        );
        assert_eq!(
            nav("surf://home"),
            OmniboxResult::Navigate(Url::parse("surf://home").unwrap())
        );
    }

    #[test]
    fn bare_hosts_upgrade_to_https() {
        assert_eq!(
            nav("example.com"),
            OmniboxResult::Navigate(Url::parse("https://example.com/").unwrap())
        );
        assert_eq!(
            nav("example.com/path"),
            OmniboxResult::Navigate(Url::parse("https://example.com/path").unwrap())
        );
    }

    #[test]
    fn loopback_uses_http() {
        assert_eq!(
            nav("localhost:3000"),
            OmniboxResult::Navigate(Url::parse("http://localhost:3000/").unwrap())
        );
        assert_eq!(
            nav("127.0.0.1"),
            OmniboxResult::Navigate(Url::parse("http://127.0.0.1/").unwrap())
        );
    }

    #[test]
    fn queries_search() {
        assert_eq!(nav("surf browser"), OmniboxResult::Search("surf browser".into()));
        assert_eq!(nav("what is 2+2"), OmniboxResult::Search("what is 2+2".into()));
        assert_eq!(nav(""), OmniboxResult::Search(String::new()));
    }

    #[test]
    fn unknown_scheme_searches() {
        // "hello:world" parses with scheme "hello"; treat as search like Chrome does for typos.
        assert_eq!(nav("hello:world"), OmniboxResult::Search("hello:world".into()));
    }

    #[test]
    fn single_word_without_dot_searches() {
        assert_eq!(nav("documentation"), OmniboxResult::Search("documentation".into()));
    }

    #[test]
    fn search_urls_for_known_engines() {
        for (engine, base) in [
            ("duckduckgo", "https://duckduckgo.com/?q=hi+there"),
            ("google", "https://www.google.com/search?q=hi+there"),
            ("bing", "https://www.bing.com/search?q=hi+there"),
        ] {
            assert_eq!(
                search_url(engine, None, "hi there"),
                OmniboxResult::Navigate(Url::parse(base).unwrap()),
                "{engine}"
            );
        }
    }

    #[test]
    fn custom_engine_template_builds_and_rejects() {
        // Placeholder substituted with an encoded query.
        assert_eq!(
            search_url("custom", Some("https://kagi.com/search?q={q}"), "hi there"),
            OmniboxResult::Navigate(Url::parse("https://kagi.com/search?q=hi%20there").unwrap())
        );
        // Percent-signs in the query cannot open a second parameter.
        assert_eq!(
            search_url("custom", Some("https://s.example/?q={q}&safe=1"), "a%26bad=2"),
            OmniboxResult::Navigate(Url::parse("https://s.example/?q=a%2526bad%3D2&safe=1").unwrap())
        );
        // A template without the placeholder, with spaces, or one that
        // does not parse falls back to a search of the raw query.
        assert_eq!(
            search_url("custom", Some("https://s.example/"), "q"),
            OmniboxResult::Search("q".into())
        );
        assert_eq!(
            search_url("custom", Some("not a url {q}"), "q"),
            OmniboxResult::Search("q".into())
        );
        assert_eq!(
            search_url("custom", Some("http://{q}"), "q"),
            OmniboxResult::Navigate(Url::parse("http://q").unwrap())
        );
    }
}
