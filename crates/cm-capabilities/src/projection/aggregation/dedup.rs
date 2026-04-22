use std::collections::HashMap;

use cm_core::Entry;
use uuid::Uuid;

use super::hex_prefix;

/// Length of the content-hash prefix used for intra-response dedup.
///
/// 16 hex characters carry 64 bits of entropy, so BLAKE3 prefix
/// collisions on realistic result-set sizes are negligible. Exposed so
/// the recall/browse formatters that render `dup_of:` annotations can
/// reference the same constant.
pub const CONTENT_HASH_DEDUP_PREFIX: usize = 16;

/// Intra-response dedup pass: map each duplicate row's id to the id of
/// the first row (the leader) that carries the same content-hash prefix.
///
/// Walks `rows` in order, indexing the first 16 hex characters of each
/// row's `content_hash` into a leader table. Rows whose prefix is
/// already in the table are duplicates: their id maps to the leader's
/// id in the returned map. The leader itself is never present in the
/// output, so callers drive rendering as:
///
/// ```ignore
/// let dedup = compute_dedup_hints(&rows);
/// for row in &rows {
///     if let Some(leader_id) = dedup.get(&row.id) {
///         // render `dup_of: <short leader id>`
///     }
/// }
/// ```
///
/// Runs in O(n) with one `HashMap` allocation plus one short-string
/// allocation per row. Order-stable: if rows 1, 2, and 3 share a
/// prefix, both rows 2 and 3 map to row 1 (not a chain).
pub fn compute_dedup_hints(rows: &[&Entry]) -> HashMap<Uuid, Uuid> {
    let mut leaders: HashMap<String, Uuid> = HashMap::new();
    let mut dupes: HashMap<Uuid, Uuid> = HashMap::new();
    for row in rows {
        let prefix = hex_prefix(&row.content_hash, CONTENT_HASH_DEDUP_PREFIX).to_owned();
        if let Some(&leader_id) = leaders.get(&prefix) {
            dupes.insert(row.id, leader_id);
        } else {
            leaders.insert(prefix, row.id);
        }
    }
    dupes
}
