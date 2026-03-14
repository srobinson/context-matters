//! Content hash deduplication logic.
//!
//! Checks whether an active (non-superseded) entry with the same BLAKE3
//! content hash already exists. Used by the write path to reject duplicate
//! inserts and by the update path to revalidate after body/kind changes.

use cm_core::CmError;
use sqlx::SqlitePool;
use uuid::Uuid;

/// Check if an active entry with the given content hash exists.
///
/// Returns `Ok(())` if no duplicate is found, or
/// `Err(CmError::DuplicateContent(existing_id))` if one exists.
///
/// Superseded entries (those with `superseded_by IS NOT NULL`) are excluded
/// from the check, allowing re-insertion of previously superseded content.
pub async fn check_duplicate(
    pool: &SqlitePool,
    content_hash: &str,
    exclude_id: Option<&str>,
) -> Result<(), CmError> {
    let existing: Option<(String,)> = if let Some(id) = exclude_id {
        // On update: exclude the entry being updated from the check
        sqlx::query_as(
            "SELECT id FROM entries \
             WHERE content_hash = ? AND superseded_by IS NULL AND id != ? \
             LIMIT 1",
        )
        .bind(content_hash)
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| CmError::Internal(e.to_string()))?
    } else {
        // On insert: check all active entries
        sqlx::query_as(
            "SELECT id FROM entries \
             WHERE content_hash = ? AND superseded_by IS NULL \
             LIMIT 1",
        )
        .bind(content_hash)
        .fetch_optional(pool)
        .await
        .map_err(|e| CmError::Internal(e.to_string()))?
    };

    if let Some((id_str,)) = existing {
        let uuid = Uuid::parse_str(&id_str)
            .map_err(|e| CmError::Internal(format!("invalid UUID in entries table: {e}")))?;
        return Err(CmError::DuplicateContent(uuid));
    }

    Ok(())
}

/// Compute a content hash for an update, given the current entry state
/// and the partial update. Returns the new hash if body or kind changed,
/// or `None` if neither changed (no revalidation needed).
pub fn recompute_hash_for_update(
    current_scope: &str,
    current_kind: &str,
    current_body: &str,
    update_kind: Option<&str>,
    update_body: Option<&str>,
) -> Option<String> {
    if update_kind.is_none() && update_body.is_none() {
        return None;
    }

    let kind = update_kind.unwrap_or(current_kind);
    let body = update_body.unwrap_or(current_body);

    let mut hasher = blake3::Hasher::new();
    hasher.update(current_scope.as_bytes());
    hasher.update(b"\0");
    hasher.update(kind.as_bytes());
    hasher.update(b"\0");
    hasher.update(body.as_bytes());
    Some(hasher.finalize().to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{create_pools, run_migrations};

    const ID1: &str = "0193a5e0-7b1a-7000-8000-000000000001";
    const ID2: &str = "0193a5e0-7b1a-7000-8000-000000000002";

    /// Helper: set up a fresh database with migrations and global scope.
    async fn setup() -> (SqlitePool, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let (wp, _rp) = create_pools(&db_path).await.unwrap();
        run_migrations(&wp).await.unwrap();

        sqlx::query("INSERT INTO scopes (path, kind, label) VALUES ('global', 'global', 'Global')")
            .execute(&wp)
            .await
            .unwrap();

        (wp, dir)
    }

    async fn insert_entry(pool: &SqlitePool, id: &str, hash: &str) {
        sqlx::query(
            "INSERT INTO entries (id, scope_path, kind, title, body, content_hash, created_by) \
             VALUES (?, 'global', 'fact', 'Title', 'Body', ?, 'test')",
        )
        .bind(id)
        .bind(hash)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn no_duplicate_on_empty_table() {
        let (pool, _dir) = setup().await;
        let result = check_duplicate(&pool, "somehash", None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn duplicate_detected_on_matching_hash() {
        let (pool, _dir) = setup().await;
        insert_entry(&pool, ID1, "hash123").await;

        let result = check_duplicate(&pool, "hash123", None).await;
        let expected_uuid = Uuid::parse_str(ID1).unwrap();
        assert!(matches!(result, Err(CmError::DuplicateContent(id)) if id == expected_uuid));
    }

    #[tokio::test]
    async fn superseded_entry_excluded_from_dedup() {
        let (pool, _dir) = setup().await;
        insert_entry(&pool, ID1, "hash123").await;
        insert_entry(&pool, ID2, "hash_other").await;

        // Supersede ID1 by pointing to ID2
        sqlx::query("UPDATE entries SET superseded_by = ? WHERE id = ?")
            .bind(ID2)
            .bind(ID1)
            .execute(&pool)
            .await
            .unwrap();

        // Same hash should now be allowed (ID1 is superseded)
        let result = check_duplicate(&pool, "hash123", None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn update_excludes_self() {
        let (pool, _dir) = setup().await;
        insert_entry(&pool, ID1, "hash123").await;

        // When updating ID1, its own hash should not be flagged as duplicate
        let result = check_duplicate(&pool, "hash123", Some(ID1)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn update_detects_collision_with_other_entry() {
        let (pool, _dir) = setup().await;
        insert_entry(&pool, ID1, "hash_a").await;
        insert_entry(&pool, ID2, "hash_b").await;

        // Updating ID2 to have ID1's hash should be a duplicate
        let result = check_duplicate(&pool, "hash_a", Some(ID2)).await;
        let expected_uuid = Uuid::parse_str(ID1).unwrap();
        assert!(matches!(result, Err(CmError::DuplicateContent(id)) if id == expected_uuid));
    }

    #[test]
    fn recompute_hash_no_change() {
        let result = recompute_hash_for_update("global", "fact", "body", None, None);
        assert!(result.is_none());
    }

    #[test]
    fn recompute_hash_body_change() {
        let hash = recompute_hash_for_update("global", "fact", "old", None, Some("new"));
        assert!(hash.is_some());
        assert_eq!(hash.as_ref().unwrap().len(), 64);

        // Different body should produce different hash
        let hash2 = recompute_hash_for_update("global", "fact", "old", None, Some("other"));
        assert_ne!(hash, hash2);
    }

    #[test]
    fn recompute_hash_kind_change() {
        let hash = recompute_hash_for_update("global", "fact", "body", Some("decision"), None);
        assert!(hash.is_some());

        // Same body but original kind should differ
        let original = recompute_hash_for_update("global", "fact", "body", None, Some("body"));
        assert_ne!(hash, original);
    }

    #[test]
    fn recompute_hash_matches_new_entry_hash() {
        use cm_core::{EntryKind, NewEntry, ScopePath};

        let new_entry = NewEntry {
            scope_path: ScopePath::parse("global").unwrap(),
            kind: EntryKind::Fact,
            title: "ignored".to_owned(),
            body: "test body".to_owned(),
            created_by: "test".to_owned(),
            meta: None,
        };

        let recomputed =
            recompute_hash_for_update("global", "fact", "test body", None, Some("test body"))
                .unwrap();

        assert_eq!(new_entry.content_hash(), recomputed);
    }
}
