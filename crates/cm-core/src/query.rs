use crate::{EntryKind, ScopePath};

/// Builder for constructing structured entry queries.
///
/// Provides a fluent API for assembling filter criteria that
/// the storage layer translates into SQL WHERE clauses.
#[derive(Debug, Clone, Default)]
pub struct QueryBuilder {
    scope_path: Option<ScopePath>,
    kinds: Vec<EntryKind>,
    tag: Option<String>,
    created_by: Option<String>,
    include_superseded: bool,
    limit: Option<u32>,
}

impl QueryBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter to entries at this exact scope path.
    pub fn scope(mut self, path: ScopePath) -> Self {
        self.scope_path = Some(path);
        self
    }

    /// Filter to entries of these kinds. Multiple kinds use OR logic.
    pub fn kinds(mut self, kinds: Vec<EntryKind>) -> Self {
        self.kinds = kinds;
        self
    }

    /// Filter to entries with this tag.
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    /// Filter to entries created by this attribution.
    pub fn created_by(mut self, created_by: impl Into<String>) -> Self {
        self.created_by = Some(created_by.into());
        self
    }

    /// Include superseded (inactive) entries in results.
    pub fn include_superseded(mut self, include: bool) -> Self {
        self.include_superseded = include;
        self
    }

    /// Set the maximum number of results.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn get_scope_path(&self) -> Option<&ScopePath> {
        self.scope_path.as_ref()
    }

    pub fn get_kinds(&self) -> &[EntryKind] {
        &self.kinds
    }

    pub fn get_tag(&self) -> Option<&str> {
        self.tag.as_deref()
    }

    pub fn get_created_by(&self) -> Option<&str> {
        self.created_by.as_deref()
    }

    pub fn get_include_superseded(&self) -> bool {
        self.include_superseded
    }

    pub fn get_limit(&self) -> Option<u32> {
        self.limit
    }
}

/// Helper for constructing FTS5 query strings.
///
/// Sanitizes user input to prevent FTS5 syntax errors while
/// preserving intended search semantics.
#[derive(Debug, Clone)]
pub struct FtsQuery {
    raw: String,
}

impl FtsQuery {
    /// Create a new FTS query from user input.
    ///
    /// Performs minimal sanitization: strips unbalanced quotes
    /// and escapes characters that could cause FTS5 syntax errors.
    /// Preserves prefix queries (`word*`), phrase queries (`"exact phrase"`),
    /// and boolean operators (`AND`, `OR`, `NOT`).
    pub fn new(input: &str) -> Self {
        Self {
            raw: sanitize_fts_input(input),
        }
    }

    /// Return the sanitized query string for use in FTS5 MATCH.
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// Build a prefix query: appends `*` to each word.
    ///
    /// Useful for "search as you type" behavior where partial
    /// word matches are desired.
    pub fn prefix_query(input: &str) -> Self {
        let terms: Vec<String> = input
            .split_whitespace()
            .filter(|w| !w.is_empty())
            .map(|w| {
                let clean = sanitize_word(w);
                if clean.ends_with('*') {
                    clean
                } else {
                    format!("{clean}*")
                }
            })
            .collect();

        Self {
            raw: terms.join(" "),
        }
    }
}

/// Sanitize a single word for FTS5: keep alphanumeric and `*` (prefix queries).
///
/// All non-alphanumeric characters (except `*`) are replaced with spaces to
/// match the unicode61 tokenizer's behavior, which treats hyphens, dots,
/// underscores, slashes, colons, dashes, and all other punctuation as token
/// separators. Without this, characters like `-` become FTS5's NOT operator,
/// `:` becomes the column filter operator, and various Unicode dashes
/// (en dash, em dash, minus sign) cause syntax errors.
fn sanitize_word(word: &str) -> String {
    word.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '*' {
                c
            } else {
                ' '
            }
        })
        .collect()
}

/// Sanitize user input for FTS5 MATCH syntax.
///
/// Preserves:
/// - Balanced quoted phrases: `"hello world"`
/// - Prefix operators: `word*`
/// - Boolean operators: AND, OR, NOT (uppercase only)
///
/// Strips:
/// - Unbalanced quotes
/// - Special characters that cause FTS5 syntax errors (parens, carets, etc.)
fn sanitize_fts_input(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Count quotes to detect unbalanced state
    let quote_count = trimmed.chars().filter(|c| *c == '"').count();
    let balanced_quotes = quote_count % 2 == 0;

    if balanced_quotes && trimmed.contains('"') {
        // Preserve quoted phrases verbatim, sanitize non-quoted portions.
        // Collect all parts (quoted phrases and sanitized words) then join with spaces.
        let mut parts: Vec<String> = Vec::new();
        let mut in_quote = false;
        let mut segment = String::new();

        for c in trimmed.chars() {
            if c == '"' {
                if in_quote {
                    // Closing quote: emit quoted phrase as a single part
                    parts.push(format!("\"{}\"", segment));
                    segment.clear();
                } else {
                    // Opening quote: flush and sanitize accumulated non-quoted text
                    if !segment.is_empty() {
                        for word in sanitize_unquoted_words(&segment) {
                            parts.push(word);
                        }
                        segment.clear();
                    }
                }
                in_quote = !in_quote;
            } else {
                segment.push(c);
            }
        }
        // Flush any trailing non-quoted text
        if !segment.is_empty() {
            for word in sanitize_unquoted_words(&segment) {
                parts.push(word);
            }
        }
        return parts.join(" ");
    }

    // Strip all quotes if unbalanced, then sanitize each word
    sanitize_unquoted_words(trimmed).join(" ")
}

/// Return sanitized words from a non-quoted segment as individual strings.
///
/// Each input word is sanitized (hyphens become spaces), then the result
/// is re-split on whitespace to flatten multi-word expansions into
/// separate search terms.
fn sanitize_unquoted_words(segment: &str) -> Vec<String> {
    segment
        .split_whitespace()
        .flat_map(|w| {
            let stripped = w.replace('"', "");
            if stripped == "AND" || stripped == "OR" || stripped == "NOT" {
                vec![stripped]
            } else {
                sanitize_word(&stripped)
                    .split_whitespace()
                    .map(String::from)
                    .collect::<Vec<_>>()
            }
        })
        .filter(|w| !w.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fts_query_simple() {
        let q = FtsQuery::new("hello world");
        assert_eq!(q.as_str(), "hello world");
    }

    #[test]
    fn fts_query_preserves_balanced_quotes() {
        let q = FtsQuery::new("\"hello world\"");
        assert_eq!(q.as_str(), "\"hello world\"");
    }

    #[test]
    fn fts_query_strips_unbalanced_quotes() {
        let q = FtsQuery::new("hello \"world");
        assert!(!q.as_str().contains('"'));
    }

    #[test]
    fn fts_query_preserves_prefix() {
        let q = FtsQuery::new("rust*");
        assert_eq!(q.as_str(), "rust*");
    }

    #[test]
    fn fts_query_preserves_boolean() {
        let q = FtsQuery::new("rust AND tokio");
        assert_eq!(q.as_str(), "rust AND tokio");
    }

    #[test]
    fn fts_prefix_query() {
        let q = FtsQuery::prefix_query("hel wor");
        assert_eq!(q.as_str(), "hel* wor*");
    }

    #[test]
    fn fts_prefix_query_no_double_star() {
        let q = FtsQuery::prefix_query("hello*");
        assert_eq!(q.as_str(), "hello*");
    }

    #[test]
    fn fts_query_empty() {
        let q = FtsQuery::new("");
        assert_eq!(q.as_str(), "");
    }

    #[test]
    fn fts_query_balanced_quotes_sanitizes_outside() {
        // Parens are FTS5 grouping syntax and become spaces in non-quoted portions
        let q = FtsQuery::new("\"hello world\" AND foo(bar)");
        assert_eq!(q.as_str(), "\"hello world\" AND foo bar");
    }

    #[test]
    fn fts_query_balanced_quotes_mixed() {
        let q = FtsQuery::new("test \"exact phrase\" other* stuff");
        assert_eq!(q.as_str(), "test \"exact phrase\" other* stuff");
    }

    #[test]
    fn fts_query_strips_carets_outside_quotes() {
        let q = FtsQuery::new("\"keep this\" ^remove");
        assert_eq!(q.as_str(), "\"keep this\" remove");
    }

    #[test]
    fn fts_query_hyphen_becomes_space() {
        // Hyphens must not reach FTS5 as the NOT operator.
        // "tools-toml" should become "tools toml" (implicit AND).
        let q = FtsQuery::new("tools-toml");
        assert_eq!(q.as_str(), "tools toml");
    }

    #[test]
    fn fts_query_multiple_hyphens() {
        let q = FtsQuery::new("my-cool-tool");
        assert_eq!(q.as_str(), "my cool tool");
    }

    #[test]
    fn fts_query_hyphen_in_phrase_preserved() {
        // Inside quoted phrases, hyphens should be preserved (tokenizer handles them)
        let q = FtsQuery::new("\"tools-toml\"");
        assert_eq!(q.as_str(), "\"tools-toml\"");
    }

    #[test]
    fn query_builder_defaults() {
        let qb = QueryBuilder::new();
        assert!(qb.get_scope_path().is_none());
        assert!(qb.get_kinds().is_empty());
        assert!(!qb.get_include_superseded());
        assert!(qb.get_limit().is_none());
    }

    #[test]
    fn query_builder_fluent() {
        let qb = QueryBuilder::new()
            .scope(ScopePath::global())
            .kinds(vec![EntryKind::Fact, EntryKind::Decision])
            .tag("rust")
            .created_by("agent:claude")
            .include_superseded(true)
            .limit(10);

        assert_eq!(qb.get_scope_path().unwrap().as_str(), "global");
        assert_eq!(qb.get_kinds().len(), 2);
        assert_eq!(qb.get_tag(), Some("rust"));
        assert_eq!(qb.get_created_by(), Some("agent:claude"));
        assert!(qb.get_include_superseded());
        assert_eq!(qb.get_limit(), Some(10));
    }
}
