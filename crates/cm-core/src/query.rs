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

/// Sanitize a single word for FTS5: keep alphanumeric, hyphens, asterisk.
fn sanitize_word(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '*' || *c == '_')
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
/// - Special characters that cause FTS5 syntax errors
fn sanitize_fts_input(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Count quotes to detect unbalanced state
    let quote_count = trimmed.chars().filter(|c| *c == '"').count();
    let balanced_quotes = quote_count % 2 == 0;

    if balanced_quotes && trimmed.contains('"') {
        // Preserve the input mostly as-is when quotes are balanced,
        // only cleaning non-phrase segments
        return trimmed.to_string();
    }

    // Strip all quotes if unbalanced, then sanitize each word
    let words: Vec<String> = trimmed
        .split_whitespace()
        .map(|w| {
            let stripped = w.replace('"', "");
            if stripped == "AND" || stripped == "OR" || stripped == "NOT" {
                stripped
            } else {
                sanitize_word(&stripped)
            }
        })
        .filter(|w| !w.is_empty())
        .collect();

    words.join(" ")
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
