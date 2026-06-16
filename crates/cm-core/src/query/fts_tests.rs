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
fn recall_auto_prefix() {
    let q = FtsQuery::recall_auto_prefix("hel wor");
    assert_eq!(q.as_str(), "hel* wor*");
}

#[test]
fn recall_auto_prefix_no_double_star() {
    let q = FtsQuery::recall_auto_prefix("hello*");
    assert_eq!(q.as_str(), "hello*");
}

#[test]
fn recall_auto_prefix_leaves_short_terms_exact() {
    let q = FtsQuery::recall_auto_prefix("io vps");
    assert_eq!(q.as_str(), "io vps*");
}

#[test]
fn recall_auto_prefix_preserves_quoted_phrases() {
    let q = FtsQuery::recall_auto_prefix("auth \"exact phrase\" migration");
    assert_eq!(q.as_str(), "auth* \"exact phrase\" migration*");
}

// Reserved-word stripping (ALP-1765 regression).
//
// Before ALP-1765, the recall prefix tier blindly starred every token, so an
// uppercase `AND`, `OR`, or `NOT` in a natural-language query produced
// `NOT*` etc. and crashed FTS5 with `syntax error near "*"`. The
// recall cascade then propagated the error instead of advancing to
// the SplitOr tier. These tests lock the stripping in.

#[test]
fn recall_auto_prefix_strips_not_in_middle() {
    let q = FtsQuery::recall_auto_prefix("foo NOT bar");
    assert_eq!(q.as_str(), "foo* bar*");
}

#[test]
fn recall_auto_prefix_strips_and_in_middle() {
    let q = FtsQuery::recall_auto_prefix("foo AND bar");
    assert_eq!(q.as_str(), "foo* bar*");
}

#[test]
fn recall_auto_prefix_strips_or_in_middle() {
    let q = FtsQuery::recall_auto_prefix("foo OR bar");
    assert_eq!(q.as_str(), "foo* bar*");
}

#[test]
fn recall_auto_prefix_strips_reserved_at_edges() {
    let q = FtsQuery::recall_auto_prefix("AND foo NOT");
    assert_eq!(q.as_str(), "foo*");
}

#[test]
fn recall_auto_prefix_only_reserved_words_yields_empty() {
    let q = FtsQuery::recall_auto_prefix("AND NOT OR");
    assert_eq!(q.as_str(), "");
}

#[test]
fn recall_auto_prefix_field_repro() {
    let q = FtsQuery::recall_auto_prefix("FTS5 sanitization hyphens NOT operators");
    assert_eq!(q.as_str(), "FTS5* sanitization* hyphens* operators*");
}

#[test]
fn recall_auto_prefix_hyphen_splits_into_two_prefix_tokens() {
    let q = FtsQuery::recall_auto_prefix("foo-bar");
    assert_eq!(q.as_str(), "foo* bar*");
}

#[test]
fn recall_auto_prefix_strips_lowercase_left_alone() {
    let q = FtsQuery::recall_auto_prefix("foo and bar or baz not qux");
    assert_eq!(q.as_str(), "foo* and* bar* or baz* not* qux*");
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
