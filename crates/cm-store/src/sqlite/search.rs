use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use cm_core::{CmError, ContentSearchPage, ContentSearchRequest, ScoredEntry};
use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Row, Sqlite};
use uuid::Uuid;

use super::CmStore;
use super::parse::{map_db_err, parse_entry};
use super::predicates::{
    push_kind_predicate, push_scope_filter, push_tag_predicate, push_where_prefix,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SearchCursor {
    rank: f64,
    ts: DateTime<Utc>,
    id: Uuid,
}

enum ContentSearchQueryMode {
    Rows,
    #[cfg(test)]
    ExplainPlan,
}

impl CmStore {
    pub(crate) async fn do_content_search(
        &self,
        request: ContentSearchRequest,
    ) -> Result<ContentSearchPage, CmError> {
        let fts_query = cm_core::FtsQuery::new(&request.query);
        let fts_str = fts_query.as_str().to_owned();

        if fts_str.is_empty() {
            return Ok(ContentSearchPage {
                items: Vec::new(),
                next_cursor: None,
            });
        }

        let cursor = request
            .cursor
            .as_deref()
            .map(decode_search_cursor)
            .transpose()?;
        let rows = self
            .fetch_content_search_rows(&request, &fts_str, cursor.as_ref())
            .await?;
        let has_more = rows.len() > request.limit as usize;
        let take_count = if has_more {
            request.limit as usize
        } else {
            rows.len()
        };

        let mut items = Vec::with_capacity(take_count);
        for row in rows.iter().take(take_count) {
            let entry = parse_entry(row)?;
            let rank: f64 = row.get("fts_rank");
            items.push(ScoredEntry {
                entry,
                score: rank as f32,
            });
        }

        let next_cursor = if has_more && take_count > 0 {
            let rank: f64 = rows[take_count - 1].get("fts_rank");
            items
                .last()
                .map(|last| encode_search_cursor(rank, &last.entry))
        } else {
            None
        };

        Ok(ContentSearchPage { items, next_cursor })
    }

    async fn fetch_content_search_rows(
        &self,
        request: &ContentSearchRequest,
        fts_str: &str,
        cursor: Option<&SearchCursor>,
    ) -> Result<Vec<sqlx::sqlite::SqliteRow>, CmError> {
        build_content_search_query(request, fts_str, cursor, ContentSearchQueryMode::Rows)
            .build()
            .fetch_all(&self.read_pool)
            .await
            .map_err(map_db_err)
    }
}

fn build_content_search_query(
    request: &ContentSearchRequest,
    fts_str: &str,
    cursor: Option<&SearchCursor>,
    mode: ContentSearchQueryMode,
) -> QueryBuilder<'static, Sqlite> {
    let mut q = QueryBuilder::<Sqlite>::new(match mode {
        ContentSearchQueryMode::Rows => {
            "SELECT e.*, f.rank AS fts_rank FROM entries e \
             JOIN entries_fts f ON e.rowid = f.rowid \
             WHERE f.entries_fts MATCH "
        }
        #[cfg(test)]
        ContentSearchQueryMode::ExplainPlan => {
            "EXPLAIN QUERY PLAN SELECT e.*, f.rank AS fts_rank FROM entries e \
             JOIN entries_fts f ON e.rowid = f.rowid \
             WHERE f.entries_fts MATCH "
        }
    });
    q.push_bind(fts_str.to_owned());
    let mut has_where = true;

    push_where_prefix(&mut q, &mut has_where);
    q.push("e.superseded_by IS NULL");
    push_scope_filter(&mut q, &mut has_where, &request.scope);
    if let Some(ref kinds) = request.kinds {
        push_kind_predicate(&mut q, &mut has_where, kinds);
    }
    if let Some(ref tags) = request.tags {
        push_tag_predicate(&mut q, &mut has_where, tags);
    }
    if let Some(cursor) = cursor {
        push_where_prefix(&mut q, &mut has_where);
        push_search_cursor_condition(&mut q, cursor);
    }

    q.push(" ORDER BY f.rank ASC, e.updated_at DESC, e.id ASC LIMIT ");
    q.push_bind(request.limit as i64 + 1);
    q
}

fn encode_search_cursor(rank: f64, entry: &cm_core::Entry) -> String {
    let payload = SearchCursor {
        rank,
        ts: entry.updated_at,
        id: entry.id,
    };
    let json = serde_json::to_string(&payload).expect("cursor serialization");
    URL_SAFE_NO_PAD.encode(json.as_bytes())
}

fn decode_search_cursor(encoded: &str) -> Result<SearchCursor, CmError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| CmError::Validation("Invalid cursor format".into()))?;

    serde_json::from_slice(&bytes).map_err(|_| CmError::Validation("Invalid cursor format".into()))
}

fn push_search_cursor_condition(query: &mut QueryBuilder<'_, Sqlite>, cursor: &SearchCursor) {
    let ts = cursor.ts.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

    query.push("(f.rank > ");
    query.push_bind(cursor.rank);
    query.push(" OR (f.rank = ");
    query.push_bind(cursor.rank);
    query.push(" AND (e.updated_at < ");
    query.push_bind(ts.clone());
    query.push(" OR (e.updated_at = ");
    query.push_bind(ts);
    query.push(" AND e.id > ");
    query.push_bind(cursor.id.to_string());
    query.push("))))");
}

#[cfg(test)]
mod tests {
    use cm_core::{ContentSearchRequest, ScopeFilter};
    use sqlx::{Row, sqlite::SqlitePoolOptions};

    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn all_scope_content_search_plan_uses_fts_without_scope_index() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(":memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE entries (
                id TEXT PRIMARY KEY,
                scope_path TEXT NOT NULL,
                kind TEXT NOT NULL,
                title TEXT NOT NULL,
                body TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                meta TEXT,
                created_by TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                superseded_by TEXT
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("CREATE INDEX idx_entries_scope ON entries(scope_path)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "CREATE VIRTUAL TABLE entries_fts USING fts5(
                title,
                body,
                content='entries',
                content_rowid='rowid'
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        let request = ContentSearchRequest {
            query: "sqlite".to_owned(),
            scope: ScopeFilter::All,
            kinds: None,
            tags: None,
            limit: 10,
            cursor: None,
        };
        let mut query = build_content_search_query(
            &request,
            "sqlite",
            None,
            ContentSearchQueryMode::ExplainPlan,
        );
        let sql = query.sql().to_owned();
        let rows = query.build().fetch_all(&pool).await.unwrap();
        let plan = rows
            .iter()
            .map(|row| row.get::<String, _>("detail"))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            sql.contains("MATCH"),
            "content search should use FTS MATCH SQL:\n{sql}"
        );
        assert!(
            !sql.contains("scope_path =")
                && !sql.contains("scope_path IN")
                && !sql.contains("scope_path LIKE"),
            "all scope search should not add a scope_path predicate:\n{sql}"
        );
        assert!(
            plan.contains("VIRTUAL TABLE INDEX"),
            "content search should use the FTS virtual table path:\n{plan}"
        );
        assert!(
            !plan.contains("idx_entries_scope"),
            "all scope search should not use idx_entries_scope:\n{plan}"
        );
    }
}
