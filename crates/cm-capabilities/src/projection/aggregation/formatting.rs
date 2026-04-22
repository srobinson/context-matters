use chrono::{DateTime, Utc};

/// First `len` bytes of `id`, safe for multi-byte UTF-8.
///
/// Intended for UUID v7 hex (32 ASCII chars without hyphens), where byte
/// indices are always char boundaries. Falls back to `floor_char_boundary`
/// so arbitrary `&str` inputs never panic. Returns the full string when
/// `len` is greater than or equal to the byte length of `id`.
pub fn hex_prefix(id: &str, len: usize) -> &str {
    let bound = id.floor_char_boundary(len.min(id.len()));
    &id[..bound]
}

/// Compact human-relative age between two timestamps.
///
/// Selects the largest unit yielding a value of at least 1 and renders it
/// without pluralisation: `<1m`, `Xm`, `Xh`, `Xd`, `Xw`, `Xmo`, `Xy`. Future
/// timestamps (`now < created_at`) collapse to `<1m`.
pub fn relative_age(created_at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let secs = now.signed_duration_since(created_at).num_seconds().max(0);
    if secs < 60 {
        return "<1m".to_owned();
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{mins}m");
    }
    let hours = mins / 60;
    if hours < 24 {
        return format!("{hours}h");
    }
    let days = hours / 24;
    if days < 7 {
        return format!("{days}d");
    }
    if days < 30 {
        return format!("{w}w", w = days / 7);
    }
    if days < 365 {
        return format!("{mo}mo", mo = days / 30);
    }
    format!("{y}y", y = days / 365)
}

/// Format an integer with comma thousands separators (`3420` -> `3,420`).
///
/// Accepts `impl Into<u64>` so callers can pass `u32`, `u64`, or any smaller
/// unsigned type without explicit casts. Used by the recall formatter for
/// token budgets (`u32`) and by the stats formatter for entry counts and
/// byte sizes (`u64`). Pure ASCII; no locale dependency.
pub fn fmt_with_commas(n: impl Into<u64>) -> String {
    let s = n.into().to_string();
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(bytes.len() + bytes.len() / 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*b as char);
    }
    out
}
