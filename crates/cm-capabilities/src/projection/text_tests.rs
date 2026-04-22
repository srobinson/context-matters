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
fn insert_highlights_em_dash_after_match_does_not_panic() {
    // Regression for the char-boundary panic: when a matched term
    // ends at a byte position whose following 2 bytes straddle a
    // multi-byte char (`—` is 3 bytes: E2 80 94), the pre-fix code
    // evaluated `&snippet[start+len..start+len+2]` and panicked on
    // a non-boundary cut. This is the exact prose that reproduced
    // the cm-0.2.2 crash on scope-filtered cx_recall.
    let snippet = "v0.2.1 — parent issue **ALP-1745** \"feat: cx_* \
                       world-class retrieval — query robustness, enrichment\"";
    let out = insert_highlights(snippet, &["retrieval"]);
    assert!(out.contains("«retrieval»"), "missing highlight: {out}");
}

#[test]
fn insert_highlights_em_dash_before_match_does_not_panic() {
    // Mirror regression for the `before_is_open` branch. Place a
    // 3-byte em dash right before the matched term so the check
    // at `start - 2` lands mid-character.
    let snippet = "alpha —beta gamma";
    let out = insert_highlights(snippet, &["beta"]);
    assert!(out.contains("«beta»"), "missing highlight: {out}");
}

#[test]
fn insert_highlights_suppresses_rebracket_around_multibyte_neighbours() {
    // Verify the "already bracketed" suppression still fires when
    // the only multi-byte content in the snippet is the guillemets
    // themselves. No panic, no double-bracketing.
    let snippet = "prefix «retrieval» tail";
    let out = insert_highlights(snippet, &["retrieval"]);
    assert_eq!(out, snippet);
}

#[test]
fn insert_highlights_cjk_around_match_does_not_panic() {
    // Broader char-boundary coverage: 3-byte CJK characters flanking
    // an ASCII match. The 2-byte lookbehind / lookahead would land
    // mid-character on the raw-slice implementation.
    let snippet = "前 alpha 後";
    let out = insert_highlights(snippet, &["alpha"]);
    assert!(out.contains("«alpha»"), "missing highlight: {out}");
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
