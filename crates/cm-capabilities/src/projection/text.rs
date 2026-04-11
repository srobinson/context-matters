//! Pure text helpers for snippet generation, frontmatter/heading stripping,
//! and query-term matching. No I/O, no allocations except where explicitly
//! noted.

/// Truncate body to a snippet, safe for multi-byte UTF-8.
///
/// Uses `floor_char_boundary` (stable since Rust 1.82) to avoid
/// panicking on multi-byte character boundaries. Tries to break
/// at a word boundary for readability.
pub fn snippet(body: &str, max_bytes: usize) -> String {
    if body.len() <= max_bytes {
        return body.to_owned();
    }
    let end = body.floor_char_boundary(max_bytes);
    match body[..end].rfind(' ') {
        Some(pos) => format!("{}...", &body[..pos]),
        None => format!("{}...", &body[..end]),
    }
}

/// Strip a leading YAML frontmatter block from a body slice.
///
/// Returns the slice after the closing `---` delimiter (plus one optional
/// trailing blank line) when the body starts with `---\n` or `---\r\n` and
/// contains a matching closing delimiter on its own line. Otherwise returns
/// the body unchanged. Unterminated frontmatter is treated as absent.
pub fn strip_yaml_frontmatter(body: &str) -> &str {
    let mut cursor = if body.starts_with("---\n") {
        4
    } else if body.starts_with("---\r\n") {
        5
    } else {
        return body;
    };

    while cursor < body.len() {
        let line_end = match body[cursor..].find('\n') {
            Some(offset) => cursor + offset,
            None => return body, // unterminated — leave body intact
        };
        let line = body[cursor..line_end].trim_end_matches('\r');
        cursor = line_end + 1;
        if line == "---" {
            // Skip one optional trailing blank line after the closing fence.
            if body[cursor..].starts_with("\r\n") {
                cursor += 2;
            } else if body[cursor..].starts_with('\n') {
                cursor += 1;
            }
            return &body[cursor..];
        }
    }
    body
}

/// Strip a leading ATX markdown heading (`# `, `## `, `### `) from a body slice.
///
/// If present, skips to the first blank line (`\n\n`) and returns the slice
/// after it. Returns the empty string when the body is a heading with no
/// subsequent prose. Returns the body unchanged when no leading heading
/// is present.
pub fn strip_leading_markdown_heading(body: &str) -> &str {
    let is_heading = body.starts_with("# ") || body.starts_with("## ") || body.starts_with("### ");
    if !is_heading {
        return body;
    }
    match body.find("\n\n") {
        Some(idx) => &body[idx + 2..],
        None => "",
    }
}

/// Find the first byte offset in `body` where any whitespace-delimited term
/// from `query` appears. Comparison is case-insensitive (ASCII lowercase).
///
/// FTS5 operators and punctuation (`AND`, `OR`, `NOT`, `"`, `*`, `(`, `)`)
/// are stripped from the query before scanning. Returns `None` when no
/// term matches or when the query is empty after stripping.
pub fn first_query_match_position(body: &str, query: &str) -> Option<usize> {
    let terms: Vec<String> = query
        .split_whitespace()
        .filter_map(|raw| {
            let cleaned: String = raw
                .chars()
                .filter(|c| !matches!(c, '"' | '*' | '(' | ')'))
                .collect();
            match cleaned.to_ascii_uppercase().as_str() {
                "AND" | "OR" | "NOT" | "" => None,
                _ => Some(cleaned.to_ascii_lowercase()),
            }
        })
        .collect();

    if terms.is_empty() {
        return None;
    }

    let body_lc = body.to_ascii_lowercase();
    terms.iter().filter_map(|t| body_lc.find(t.as_str())).min()
}

/// Extract a `max_bytes` window around byte offset `start` in `body`, safe
/// for multi-byte UTF-8.
///
/// Centres the window on `start`, aligns both edges to UTF-8 char boundaries
/// via `floor_char_boundary`, extends the left edge backward to the nearest
/// preceding space, trims the right edge forward to the nearest preceding
/// space when it does not reach end-of-body, prepends `... ` when the window
/// does not start at position 0, and appends `...` when the window does not
/// reach end-of-body.
pub fn snippet_around(body: &str, start: usize, max_bytes: usize) -> String {
    if body.len() <= max_bytes {
        return body.to_owned();
    }
    let start = start.min(body.len());
    let half = max_bytes / 2;
    let left_ideal = start.saturating_sub(half);
    // If the centred window would overflow the right edge, shift it leftward
    // so the full budget remains usable.
    let (left_raw, right_raw) = if left_ideal + max_bytes > body.len() {
        (body.len() - max_bytes, body.len())
    } else {
        (left_ideal, left_ideal + max_bytes)
    };

    let left_raw = body.floor_char_boundary(left_raw);
    let right_raw = body.floor_char_boundary(right_raw);

    // Back left up to a word boundary (just past a preceding space).
    let left = if left_raw == 0 {
        0
    } else {
        body[..left_raw]
            .rfind(' ')
            .map(|p| p + 1)
            .unwrap_or(left_raw)
    };

    // Trim right back to a word boundary unless already at end-of-body.
    let right = if right_raw >= body.len() {
        body.len()
    } else {
        body[left..right_raw]
            .rfind(' ')
            .map(|p| left + p)
            .unwrap_or(right_raw)
    };

    let slice = &body[left..right];
    let prefix = if left > 0 { "... " } else { "" };
    let suffix = if right < body.len() { "..." } else { "" };
    format!("{prefix}{slice}{suffix}")
}

/// Smart snippet generator: strip leading YAML frontmatter, strip a leading
/// markdown heading, then extract a `max_bytes` window centred on the first
/// query-term match. Falls back to the start of the stripped body when
/// `query` is `None` or no term matches.
pub fn smart_snippet(body: &str, query: Option<&str>, max_bytes: usize) -> String {
    let body = strip_yaml_frontmatter(body);
    let body = strip_leading_markdown_heading(body);
    let start = match query {
        Some(q) => first_query_match_position(body, q).unwrap_or(0),
        None => 0,
    };
    snippet_around(body, start, max_bytes)
}

/// Rough token estimate: ~4 characters per token for English text.
pub fn estimate_tokens(text: &str) -> u32 {
    (text.len() as u32).div_ceil(4)
}

/// Collapse every run of ASCII whitespace in `s` to a single space and trim
/// leading and trailing whitespace.
///
/// Used by the YAML-text formatters to keep smart-snippet output on a single
/// line even when the source body contains embedded newlines. Both the
/// `BrowseResult` and `RecallResult` formatters depend on this invariant.
pub fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_ws = false;
    for ch in s.chars() {
        if ch.is_ascii_whitespace() {
            if !in_ws && !out.is_empty() {
                out.push(' ');
            }
            in_ws = true;
        } else {
            in_ws = false;
            out.push(ch);
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippet_short_text_unchanged() {
        assert_eq!(snippet("hello world", 200), "hello world");
    }

    #[test]
    fn snippet_truncates_at_word_boundary() {
        let long_text = "a ".repeat(150);
        let result = snippet(&long_text, 200);
        assert!(result.ends_with("..."));
        assert!(result.len() <= 210);
    }

    #[test]
    fn estimate_tokens_rough_accuracy() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcdefgh"), 2);
        assert_eq!(estimate_tokens("abc"), 1);
    }

    #[test]
    fn strip_yaml_frontmatter_removes_leading_block() {
        let input = "---\nname: foo\ndate: 2026-01-01\n---\nBody starts here.";
        assert_eq!(strip_yaml_frontmatter(input), "Body starts here.");

        // CRLF line endings.
        let crlf = "---\r\nname: foo\r\n---\r\nBody after CRLF.";
        assert_eq!(strip_yaml_frontmatter(crlf), "Body after CRLF.");

        // Trailing blank line after closing fence is consumed.
        let with_blank = "---\nkey: value\n---\n\nReal body.";
        assert_eq!(strip_yaml_frontmatter(with_blank), "Real body.");
    }

    #[test]
    fn strip_yaml_frontmatter_noop_when_absent() {
        let input = "No frontmatter here.\nJust plain text.";
        assert_eq!(strip_yaml_frontmatter(input), input);

        // Looks like a fence but not on the first line.
        let not_first = "intro\n---\nfoo\n---\nbody";
        assert_eq!(strip_yaml_frontmatter(not_first), not_first);

        // Unterminated frontmatter is treated as absent.
        let unterminated = "---\nkey: value\nno closing fence";
        assert_eq!(strip_yaml_frontmatter(unterminated), unterminated);
    }

    #[test]
    fn strip_leading_markdown_heading_removes_h1_h2_h3() {
        assert_eq!(
            strip_leading_markdown_heading("# Title\n\nbody prose"),
            "body prose"
        );
        assert_eq!(
            strip_leading_markdown_heading("## Subsection\n\nprose"),
            "prose"
        );
        assert_eq!(
            strip_leading_markdown_heading("### H3 heading\n\nprose"),
            "prose"
        );
        // Plain body is unchanged.
        assert_eq!(strip_leading_markdown_heading("plain body"), "plain body");
        // Heading with no subsequent prose yields empty.
        assert_eq!(strip_leading_markdown_heading("# Lonely"), "");
    }

    #[test]
    fn first_query_match_position_finds_first_term() {
        let body = "The quick brown fox jumps over the lazy dog.";
        assert_eq!(first_query_match_position(body, "brown"), Some(10));
        // Case-insensitive.
        assert_eq!(first_query_match_position(body, "BROWN"), Some(10));
        // Multiple terms — returns earliest match.
        assert_eq!(first_query_match_position(body, "lazy brown"), Some(10));
        // No match.
        assert_eq!(first_query_match_position(body, "cat"), None);
    }

    #[test]
    fn first_query_match_position_strips_fts_operators() {
        let body = "The quick brown fox jumps over the lazy dog.";
        assert_eq!(first_query_match_position(body, "brown OR fox"), Some(10),);
        assert_eq!(first_query_match_position(body, "(jumps*)"), Some(20),);
        // AND is dropped, so only the real terms drive the match.
        assert_eq!(first_query_match_position(body, "AND lazy"), Some(35),);
        // Query of only operators → no terms → None.
        assert_eq!(first_query_match_position(body, "AND OR NOT"), None);
    }

    #[test]
    fn snippet_around_centers_on_start_and_adds_ellipsis() {
        let body = "aaa bbb ccc ddd eee fff ggg hhh iii jjj kkk lll mmm nnn ooo ppp qqq rrr sss ttt uuu vvv www xxx yyy zzz";
        let start = body.find("mmm").expect("contains mmm");
        let result = snippet_around(body, start, 30);
        assert!(result.contains("mmm"), "result={result}");
        assert!(result.starts_with("... "), "result={result}");
        assert!(result.ends_with("..."), "result={result}");
        // 30-byte window plus "... " and "..." bookends.
        assert!(result.len() <= 40, "len={} result={result}", result.len());
    }

    #[test]
    fn snippet_around_utf8_safe_multibyte_chars() {
        // Multi-byte UTF-8: CJK (3 bytes) and emoji (4 bytes).
        let unit = "前半のテキスト 🎉🎉🎉 後半のテキスト ";
        let body = unit.repeat(20);
        let start = body.len() / 2;
        // Must not panic on char-boundary crossings.
        let result = snippet_around(&body, start, 50);
        assert!(!result.is_empty());
        // Result must itself be valid UTF-8 (String guarantees this).
        assert!(result.is_char_boundary(0));
        assert!(result.is_char_boundary(result.len()));
    }

    #[test]
    fn collapse_whitespace_squashes_newlines_and_runs() {
        assert_eq!(collapse_whitespace("a\n\nb   c\n"), "a b c");
        assert_eq!(collapse_whitespace("  leading"), "leading");
        assert_eq!(collapse_whitespace("trailing\n"), "trailing");
        assert_eq!(collapse_whitespace(""), "");
    }

    #[test]
    fn smart_snippet_session_log_case() {
        let body = "---\n\
                    session: nancy-ALP-1725-iter1\n\
                    date: 2026-04-11\n\
                    agent: claude-opus-4-6\n\
                    ---\n\
                    # Session summary\n\
                    \n\
                    Worked on cx_* MCP payload redesign. Implemented smart_snippet \
                    helper with frontmatter stripping so recall snippets surface \
                    real narrative prose instead of YAML boilerplate.";
        let result = smart_snippet(body, None, 200);
        assert!(
            !result.contains("session: nancy"),
            "YAML leaked into snippet: {result}"
        );
        assert!(
            !result.contains("date: 2026"),
            "YAML date leaked into snippet: {result}"
        );
        assert!(
            !result.starts_with("# "),
            "Markdown heading leaked into snippet: {result}"
        );
        assert!(
            result.contains("Worked on") || result.contains("smart_snippet"),
            "Narrative missing from snippet: {result}"
        );
    }
}
