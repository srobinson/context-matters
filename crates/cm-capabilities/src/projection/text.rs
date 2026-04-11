//! Pure text helpers for snippet generation, frontmatter/heading stripping,
//! and query-term matching. No I/O, no allocations except where explicitly
//! noted.

/// Controls whether [`smart_snippet`] wraps query-term matches in visual
/// markers after the window is computed.
///
/// `None` keeps the current behaviour (plain text). `Bracketed` wraps
/// each matched token in YAML-safe guillemets `«…»` (U+00AB / U+00BB)
/// so the match is visible in the rendered snippet. The wrapping runs
/// on the final windowed snippet, not the full body, so only matches
/// inside the `max_bytes` window get brackets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightStyle {
    None,
    Bracketed,
}

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
///
/// `style` controls the post-window highlighting pass. `HighlightStyle::None`
/// returns the window verbatim. `HighlightStyle::Bracketed` wraps each
/// query-term match inside the final window in guillemets (`«…»`). Bracket
/// insertion runs only when both `style == Bracketed` and `query.is_some()`.
/// If the insertion pushes the result past `max_bytes` the tail is trimmed
/// at a bracket-safe cut point; see [`truncate_respecting_brackets`].
pub fn smart_snippet(
    body: &str,
    query: Option<&str>,
    style: HighlightStyle,
    max_bytes: usize,
) -> String {
    let body = strip_yaml_frontmatter(body);
    let body = strip_leading_markdown_heading(body);
    let start = match query {
        Some(q) => first_query_match_position(body, q).unwrap_or(0),
        None => 0,
    };
    let window = snippet_around(body, start, max_bytes);
    if style != HighlightStyle::Bracketed {
        return window;
    }
    let Some(q) = query else {
        return window;
    };
    let terms = highlight_terms(q);
    if terms.is_empty() {
        return window;
    }
    let term_refs: Vec<&str> = terms.iter().map(String::as_str).collect();
    let highlighted = insert_highlights(&window, &term_refs);
    truncate_respecting_brackets(&highlighted, max_bytes)
}

/// Extract highlight-ready terms from a raw query string. Mirrors the
/// token-stripping logic in [`first_query_match_position`] so the two
/// stay in lock-step: FTS5 operators (`AND`, `OR`, `NOT`), bare
/// quantifier punctuation (`"`, `*`, `(`, `)`), and empty tokens are
/// dropped. Terms are lowercased for case-insensitive matching inside
/// [`insert_highlights`].
fn highlight_terms(query: &str) -> Vec<String> {
    query
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
        .collect()
}

/// Walk `snippet` and wrap each case-insensitive occurrence of any
/// term in `query_terms` with `«…»` guillemets (U+00AB / U+00BB).
///
/// * Casing of the body is preserved — `insert_highlights("Hello
///   WORLD", &["world"])` returns `"Hello «WORLD»"`.
/// * Empty terms are skipped; the caller is responsible for filtering
///   but the helper tolerates them defensively.
/// * Double-bracketing is suppressed: if a match is already surrounded
///   by `«` and `»` (byte-exact adjacent neighbours) the helper leaves
///   it alone. This matters when the caller re-highlights a snippet
///   that already went through one pass.
/// * Spans are extracted in a single forward scan, which means two
///   overlapping terms favour whichever one the scan hits first. All
///   current callers use disjoint terms, so this is a non-issue in
///   production, but worth documenting.
pub fn insert_highlights(snippet: &str, query_terms: &[&str]) -> String {
    let terms: Vec<String> = query_terms
        .iter()
        .filter(|t| !t.is_empty())
        .map(|t| t.to_ascii_lowercase())
        .collect();
    if terms.is_empty() {
        return snippet.to_owned();
    }

    let snippet_lc = snippet.to_ascii_lowercase();
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut cursor = 0;
    while cursor < snippet_lc.len() {
        let rest = &snippet_lc[cursor..];
        let earliest = terms
            .iter()
            .filter_map(|t| rest.find(t.as_str()).map(|i| (cursor + i, t.len())))
            .min_by_key(|(start, _)| *start);
        let Some((start, len)) = earliest else {
            break;
        };
        let before_is_open = start >= 2 && &snippet[start - 2..start] == "«";
        let after_is_close =
            start + len + 2 <= snippet.len() && &snippet[start + len..start + len + 2] == "»";
        if !(before_is_open && after_is_close) {
            spans.push((start, len));
        }
        cursor = start + len;
    }

    if spans.is_empty() {
        return snippet.to_owned();
    }

    // Each span costs 4 extra bytes (2 per guillemet). Pre-size the
    // output to avoid reallocations on snippets with many matches.
    let mut out = String::with_capacity(snippet.len() + spans.len() * 4);
    let mut prev = 0;
    for (start, len) in spans {
        out.push_str(&snippet[prev..start]);
        out.push('«');
        out.push_str(&snippet[start..start + len]);
        out.push('»');
        prev = start + len;
    }
    out.push_str(&snippet[prev..]);
    out
}

/// Truncate `s` to at most `max_bytes`, guaranteeing that the cut
/// point never lands inside a `«…»` guillemet pair.
///
/// Walks backward from `floor_char_boundary(max_bytes)` until the
/// head slice has balanced `«`/`»` counts. On an imbalance we step
/// back to just before the last unclosed `«`, dropping the partial
/// bracket entirely rather than leaving a dangling opener.
fn truncate_respecting_brackets(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_owned();
    }
    let mut end = s.floor_char_boundary(max_bytes);
    loop {
        let head = &s[..end];
        let opens = head.matches('«').count();
        let closes = head.matches('»').count();
        if opens == closes {
            break;
        }
        match head.rfind('«') {
            Some(pos) => end = pos,
            None => break,
        }
    }
    s[..end].to_owned()
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
    fn insert_highlights_single_match() {
        // Canonical happy path: a single term inside a plain snippet.
        // Output wraps the match in U+00AB / U+00BB and leaves the rest
        // of the string untouched.
        assert_eq!(
            insert_highlights("hello world", &["world"]),
            "hello «world»"
        );
    }

    #[test]
    fn insert_highlights_multi_match() {
        // All occurrences of a recurring term get bracketed, including
        // the very first and the very last, and the surrounding spaces
        // are preserved.
        assert_eq!(
            insert_highlights("foo foo foo", &["foo"]),
            "«foo» «foo» «foo»"
        );
    }

    #[test]
    fn insert_highlights_case_insensitive() {
        // Search is case-insensitive but output preserves the casing
        // of the body. A lowercase query term matches an uppercase body
        // span and the brackets wrap "WORLD" verbatim.
        assert_eq!(
            insert_highlights("Hello WORLD", &["world"]),
            "Hello «WORLD»"
        );
    }

    #[test]
    fn insert_highlights_no_match() {
        // Zero matches means zero allocations past the initial clone:
        // body comes out byte-identical.
        assert_eq!(insert_highlights("hello", &["xyz"]), "hello");
    }

    #[test]
    fn insert_highlights_skips_empty_terms() {
        // Defensive: an empty term would match every position under
        // str::find (byte 0). The helper must filter it out so the
        // output does not sprout `«»` guillemets around every char.
        assert_eq!(insert_highlights("hello", &[""]), "hello");
        assert_eq!(
            insert_highlights("hello world", &["", "world"]),
            "hello «world»"
        );
    }

    #[test]
    fn insert_highlights_avoids_double_bracketing() {
        // Re-highlighting an already-highlighted snippet is a no-op
        // for the terms that are already wrapped. This matters when
        // the caller chains two highlight passes (e.g. history-aware
        // rendering that does not want to re-emit brackets).
        let once = insert_highlights("hello world", &["world"]);
        assert_eq!(once, "hello «world»");
        assert_eq!(insert_highlights(&once, &["world"]), once);
    }

    #[test]
    fn smart_snippet_none_style_leaves_body_unchanged() {
        // Regression guard: the pre-ALP-1749 call shape (style = None)
        // must still return exactly the `snippet_around`-windowed body
        // with no guillemets introduced. The smart_snippet_session_log
        // test already covers the None-query branch; this one pins the
        // Some-query branch so the highlighting pass is gated only by
        // the style parameter.
        let body = "the quick brown fox jumps over the lazy dog";
        let result = smart_snippet(body, Some("brown"), HighlightStyle::None, 200);
        assert_eq!(result, body);
        assert!(!result.contains('«'));
        assert!(!result.contains('»'));
    }

    #[test]
    fn smart_snippet_bracketed_respects_byte_budget() {
        // A long body with many matches. The Bracketed style inserts
        // 4 extra bytes per match. The post-insertion result must still
        // fit inside `max_bytes`; truncate_respecting_brackets handles
        // the overflow, and never cuts inside a `«…»` pair.
        let body = "alpha beta ".repeat(60); // ~660 bytes, many "beta" matches
        let max_bytes = 120;
        let result = smart_snippet(&body, Some("beta"), HighlightStyle::Bracketed, max_bytes);
        assert!(
            result.len() <= max_bytes,
            "result len {} exceeds budget {}: {result}",
            result.len(),
            max_bytes,
        );
        // Any '«' emitted must be closed by a '»' — the truncation
        // helper drops dangling openers.
        let opens = result.matches('«').count();
        let closes = result.matches('»').count();
        assert_eq!(
            opens, closes,
            "unbalanced guillemets in truncated result: {result}",
        );
        // And at least one match should survive at the budget of 120
        // bytes, otherwise the test is not exercising the highlight
        // pass at all.
        assert!(
            result.contains("«beta»"),
            "expected at least one highlighted beta in: {result}",
        );
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
        let result = smart_snippet(body, None, HighlightStyle::None, 200);
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
