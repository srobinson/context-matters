use super::FtsQuery;

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
fn fts_query_operator_only_yields_empty() {
    for input in ["AND", "OR", "NOT", "AND OR NOT"] {
        let q = FtsQuery::new(input);
        assert_eq!(q.as_str(), "", "{input}");
    }
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

// Reserved-word stripping (ALP-1765 regression).
//
// Before the fix, `prefix_query` blindly starred every token, so an
// uppercase `AND`, `OR`, or `NOT` in a natural-language query produced
// `NOT*` etc. and crashed FTS5 with `syntax error near "*"`. The
// recall cascade then propagated the error instead of advancing to
// the SplitOr tier. These tests lock the stripping in.

#[test]
fn fts_prefix_query_strips_not_in_middle() {
    let q = FtsQuery::prefix_query("foo NOT bar");
    assert_eq!(q.as_str(), "foo* bar*");
}

#[test]
fn fts_prefix_query_strips_and_in_middle() {
    let q = FtsQuery::prefix_query("foo AND bar");
    assert_eq!(q.as_str(), "foo* bar*");
}

#[test]
fn fts_prefix_query_strips_or_in_middle() {
    let q = FtsQuery::prefix_query("foo OR bar");
    assert_eq!(q.as_str(), "foo* bar*");
}

#[test]
fn fts_prefix_query_strips_reserved_at_edges() {
    let q = FtsQuery::prefix_query("AND foo NOT");
    assert_eq!(q.as_str(), "foo*");
}

#[test]
fn fts_prefix_query_only_reserved_words_yields_empty() {
    let q = FtsQuery::prefix_query("AND NOT OR");
    assert_eq!(q.as_str(), "");
}

#[test]
fn fts_prefix_query_field_repro() {
    let q = FtsQuery::prefix_query("FTS5 sanitization hyphens NOT operators");
    assert_eq!(q.as_str(), "FTS5* sanitization* hyphens* operators*");
}

#[test]
fn fts_prefix_query_hyphen_splits_into_two_prefix_tokens() {
    let q = FtsQuery::prefix_query("foo-bar");
    assert_eq!(q.as_str(), "foo* bar*");
}

#[test]
fn fts_prefix_query_strips_lowercase_left_alone() {
    let q = FtsQuery::prefix_query("foo and bar or baz not qux");
    assert_eq!(q.as_str(), "foo* and* bar* or* baz* not* qux*");
}

#[test]
fn fts_query_empty() {
    let q = FtsQuery::new("");
    assert_eq!(q.as_str(), "");
}

#[test]
fn split_or_multi_word_joins_with_or() {
    let q = FtsQuery::split_or_query("context matters recent work");
    assert_eq!(q.as_str(), "context OR matters OR recent OR work");
}

#[test]
fn split_or_dedupes_case_insensitive() {
    let q = FtsQuery::split_or_query("Rust rust RUST");
    assert_eq!(q.as_str(), "Rust");
}

#[test]
fn split_or_caps_at_eight() {
    let q = FtsQuery::split_or_query("a b c d e f g h i j k l");
    assert_eq!(q.as_str(), "a OR b OR c OR d OR e OR f OR g OR h");
}

#[test]
fn split_or_strips_reserved_words() {
    let q = FtsQuery::split_or_query("foo AND bar OR baz");
    assert_eq!(q.as_str(), "foo OR bar OR baz");
}

#[test]
fn split_or_empty_input_returns_empty() {
    let q = FtsQuery::split_or_query("");
    assert_eq!(q.as_str(), "");
}

#[test]
fn fts_query_balanced_quotes_sanitizes_outside() {
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
    let q = FtsQuery::new("\"tools-toml\"");
    assert_eq!(q.as_str(), "\"tools-toml\"");
}
