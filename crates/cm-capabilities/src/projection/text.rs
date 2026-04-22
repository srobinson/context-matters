//! Pure text helpers for snippet generation, frontmatter/heading stripping,
//! and query-term matching. No I/O, no allocations except where explicitly
//! noted.

/// Maximum snippet width (bytes) shown per row across every view
/// formatter (`browse`, `recall`, `web_view`). Sized to fit a
/// prose-heavy line within one wide terminal row without wrap, and
/// small enough that the bracket-insertion pass in [`smart_snippet`]
/// has headroom before the truncate fallback kicks in.
pub const SNIPPET_MAX_BYTES: usize = 200;

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
        // Raw byte slicing would panic when the 2 bytes before or after
        // the match straddle a multi-byte char boundary (e.g. an em dash
        // `—` encoded as 3 bytes sitting across the cut point). `str::get`
        // returns `None` on non-boundary ranges, so a non-guillemet
        // neighbour is treated as "not already bracketed" and highlighting
        // proceeds. The guillemets themselves are 2 bytes each in UTF-8
        // (`«` = `C2 AB`, `»` = `C2 BB`), so a genuine bracket neighbour
        // is still detected by the equality check.
        let before_is_open = snippet
            .get(start.saturating_sub(2)..start)
            .is_some_and(|s| s == "«")
            && start >= 2;
        let after_is_close = snippet
            .get(start + len..start + len + 2)
            .is_some_and(|s| s == "»");
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
#[path = "text_tests.rs"]
mod text_tests;
