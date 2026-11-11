//! EasyList-to-WebKit content-blocker translation, delegated to the
//! adblock crate's own converter (feature `content-blocking`). Its
//! output respects the restricted url-filter grammar WKContentRuleList
//! actually compiles: no groups, no disjunctions, `^` as separator.
//! Filters the converter cannot express are skipped (degrade to allow),
//! never silently widened.

use adblock::content_blocking::CbRuleEquivalent;
use adblock::lists::ParseOptions;
use serde_json::Value;

/// Convert one EasyList-style filter line into zero or more WebKit
/// content-blocker rules. Comments, cosmetic filters, and unsupported
/// syntax produce no rules.
pub fn line_to_rules(line: &str) -> Vec<Value> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('!') || line.starts_with('#') {
        return Vec::new();
    }
    // debug = true keeps raw_line, which the content-blocking converter
    // requires (NeedsDebugMode otherwise).
    let filter = match adblock::filters::network::NetworkFilter::parse(line, true, ParseOptions::default()) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let equivalent = match CbRuleEquivalent::try_from(filter) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    equivalent
        .into_iter()
        .filter_map(|rule| serde_json::to_value(&rule).ok())
        .collect()
}

/// Convert a whole filter list. Untranslatable lines are skipped.
pub fn to_content_blocker_json<'a>(lines: impl IntoIterator<Item = &'a str>) -> String {
    let rules: Vec<Value> = lines.into_iter().flat_map(line_to_rules).collect();
    Value::Array(rules).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_anchor_produces_block_rule() {
        let rules = line_to_rules("||ads.example^");
        assert!(!rules.is_empty(), "||host^ must translate");
        assert_eq!(rules[0]["action"]["type"], "block");
        let filter = rules[0]["trigger"]["url-filter"].as_str().unwrap();
        assert!(
            !filter.contains("(?:"),
            "WebKit url-filter has no non-capturing groups/disjunctions: {filter}"
        );
    }

    #[test]
    fn exception_rule_uses_ignore_previous() {
        let rules = line_to_rules("@@||good.example^");
        assert!(!rules.is_empty());
        assert_eq!(rules[0]["action"]["type"], "ignore-previous-rules");
    }

    #[test]
    fn unsupported_syntax_is_skipped() {
        assert!(line_to_rules("##.banner").is_empty());
        assert!(line_to_rules("! comment").is_empty());
        assert!(line_to_rules("").is_empty());
    }

    #[test]
    fn json_assembles() {
        let json = to_content_blocker_json(["||ads.example^", "! skip", "||good.example^"]);
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 2);
    }
}
