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
}
