/// Helper for constructing FTS5 query strings.
///
/// Sanitizes user input to prevent FTS5 syntax errors while
/// preserving intended search semantics.
#[derive(Debug, Clone)]
pub struct FtsQuery {
    raw: String,
}

const RECALL_AUTO_PREFIX_MIN_CHARS: usize = 3;

#[derive(Clone, Copy)]
enum UnquotedMode {
    Explicit,
    RecallAutoPrefix,
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
            raw: sanitize_fts_input(input, UnquotedMode::Explicit),
        }
    }

    /// Return the sanitized query string for use in FTS5 MATCH.
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// Build the recall prefix query used by the FTS cascade.
    ///
    /// This preserves the default multi-term FTS5 AND semantics while adding
    /// prefix recall for whole words that benefit from it:
    ///
    /// * Unquoted sanitized tokens with at least three chars get a trailing
    ///   `*`, so `migration` matches `migrations`.
    /// * One and two char tokens remain exact to avoid broad matches.
    /// * Tokens that already end in `*` are passed through unchanged.
    /// * Balanced quoted phrases are preserved literally.
    /// * FTS5 reserved words `AND`, `OR`, `NOT` (uppercase only, as FTS5
    ///   interprets them) are stripped after sanitization.
    ///
    /// Reserved-word stripping was added in ALP-1765 after the recall
    /// cascade's Prefix tier was found to crash on any natural-language
    /// query containing an uppercase `AND`, `OR`, or `NOT`. The same
    /// stripping shipped in [`split_or_query`] under ALP-1746 but was not
    /// backported to this constructor when the cascade was wired up.
    pub fn recall_auto_prefix(input: &str) -> Self {
        Self {
            raw: sanitize_fts_input(input, UnquotedMode::RecallAutoPrefix),
        }
    }

    /// Build a split-OR query: joins sanitized tokens with `OR` instead
    /// of the default implicit AND.
    ///
    /// Used by the recall fallback cascade's broadest tier, where the
    /// narrower exact and prefix queries have returned nothing and the
    /// goal is to surface any row that mentions any of the query terms.
    ///
    /// Semantics:
    ///
    /// * Tokens are sanitized with [`sanitize_word`] (hyphens and other
    ///   punctuation become spaces, then re-split) so `foo-bar` yields
    ///   two terms, `foo` and `bar`.
    /// * FTS5 reserved words `AND`, `OR`, `NOT` (uppercase only, as FTS5
    ///   interprets them) are stripped; joining them with `OR` would
    ///   otherwise produce a syntax error like `foo OR OR bar`.
    /// * Tokens are deduplicated case-insensitively, preserving the first
    ///   casing encountered, so `Rust rust RUST` collapses to `Rust`.
    /// * The result is capped at 8 terms to keep the FTS5 query plan cost
    ///   bounded; terms beyond the cap are truncated silently.
    /// * Empty input (or input that sanitizes to nothing) returns an
    ///   empty raw string.
    pub fn split_or_query(input: &str) -> Self {
        const MAX_TERMS: usize = 8;
        let mut terms: Vec<String> = Vec::new();

        'outer: for raw_word in input.split_whitespace() {
            for token in sanitize_word(raw_word).split_whitespace() {
                if is_fts_operator_token(token) {
                    continue;
                }
                if terms.iter().any(|t| t.eq_ignore_ascii_case(token)) {
                    continue;
                }
                terms.push(token.to_string());
                if terms.len() == MAX_TERMS {
                    break 'outer;
                }
            }
        }

        Self {
            raw: terms.join(" OR "),
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

fn is_fts_operator_token(token: &str) -> bool {
    matches!(token, "AND" | "OR" | "NOT")
}

fn empty_operator_only_query(query: String) -> String {
    if query.split_whitespace().all(is_fts_operator_token) {
        String::new()
    } else {
        query
    }
}

fn recall_auto_prefix_term(token: &str) -> String {
    if token.ends_with('*') || token.chars().count() < RECALL_AUTO_PREFIX_MIN_CHARS {
        token.to_owned()
    } else {
        format!("{token}*")
    }
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
fn sanitize_fts_input(input: &str, mode: UnquotedMode) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Count quotes to detect unbalanced state.
    let quote_count = trimmed.chars().filter(|c| *c == '"').count();
    let balanced_quotes = quote_count % 2 == 0;

    if balanced_quotes && trimmed.contains('"') {
        let mut parts: Vec<String> = Vec::new();
        let mut in_quote = false;
        let mut segment = String::new();

        for c in trimmed.chars() {
            if c == '"' {
                if in_quote {
                    parts.push(format!("\"{}\"", segment));
                    segment.clear();
                } else if !segment.is_empty() {
                    for word in sanitize_unquoted_words(&segment, mode) {
                        parts.push(word);
                    }
                    segment.clear();
                }
                in_quote = !in_quote;
            } else {
                segment.push(c);
            }
        }

        if !segment.is_empty() {
            for word in sanitize_unquoted_words(&segment, mode) {
                parts.push(word);
            }
        }
        return empty_operator_only_query(parts.join(" "));
    }

    empty_operator_only_query(sanitize_unquoted_words(trimmed, mode).join(" "))
}

/// Return sanitized words from a non-quoted segment as individual strings.
///
/// Each input word is sanitized (hyphens become spaces), then the result
/// is re-split on whitespace to flatten multi-word expansions into
/// separate search terms.
fn sanitize_unquoted_words(segment: &str, mode: UnquotedMode) -> Vec<String> {
    segment
        .split_whitespace()
        .flat_map(|w| {
            let stripped = w.replace('"', "");
            sanitize_unquoted_word(&stripped, mode)
        })
        .filter(|w| !w.is_empty())
        .collect()
}

fn sanitize_unquoted_word(word: &str, mode: UnquotedMode) -> Vec<String> {
    if is_fts_operator_token(word) {
        return match mode {
            UnquotedMode::Explicit => vec![word.to_owned()],
            UnquotedMode::RecallAutoPrefix => Vec::new(),
        };
    }

    sanitize_word(word)
        .split_whitespace()
        .map(|token| match mode {
            UnquotedMode::Explicit => token.to_owned(),
            UnquotedMode::RecallAutoPrefix => recall_auto_prefix_term(token),
        })
        .collect()
}
