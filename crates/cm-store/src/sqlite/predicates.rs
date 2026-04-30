use cm_core::{EntryKind, ScopeFilter, ScopePath};
use sqlx::{QueryBuilder, Sqlite};

pub(crate) fn push_where_prefix(query: &mut QueryBuilder<'_, Sqlite>, has_where: &mut bool) {
    if *has_where {
        query.push(" AND ");
    } else {
        query.push(" WHERE ");
        *has_where = true;
    }
}

pub(crate) fn push_scope_filter(
    query: &mut QueryBuilder<'_, Sqlite>,
    has_where: &mut bool,
    scope: &ScopeFilter,
) {
    match scope {
        ScopeFilter::All => {}
        ScopeFilter::Exact(scope_path) => {
            push_exact_scope(query, has_where, scope_path);
        }
        ScopeFilter::AncestorWalk(scope_path) => {
            push_scope_set(query, has_where, scope_path.ancestors());
        }
        ScopeFilter::Subtree(scope_path) => {
            let lower_bound = format!("{}/", scope_path.as_str());
            let upper_bound = format!("{}0", scope_path.as_str());

            push_where_prefix(query, has_where);
            query.push("(scope_path = ");
            query.push_bind(scope_path.as_str().to_owned());
            query.push(" OR (scope_path LIKE ");
            query.push_bind(scope_path.as_str().to_owned());
            query.push(" || '/%' AND scope_path >= ");
            query.push_bind(lower_bound);
            query.push(" AND scope_path < ");
            query.push_bind(upper_bound);
            query.push("))");
        }
        ScopeFilter::Set(scope_paths) => {
            push_scope_set(query, has_where, scope_paths.iter().map(ScopePath::as_str));
        }
    }
}

pub(crate) fn push_kind_predicate(
    query: &mut QueryBuilder<'_, Sqlite>,
    has_where: &mut bool,
    kinds: &[EntryKind],
) {
    if kinds.is_empty() {
        return;
    }

    push_where_prefix(query, has_where);
    query.push("e.kind IN (");
    {
        let mut separated = query.separated(", ");
        for kind in kinds {
            separated.push_bind(kind.as_str().to_owned());
        }
    }
    query.push(")");
}

pub(crate) fn push_tag_predicate(
    query: &mut QueryBuilder<'_, Sqlite>,
    has_where: &mut bool,
    tags: &[String],
) {
    if tags.is_empty() {
        return;
    }

    push_where_prefix(query, has_where);
    query.push("EXISTS (SELECT 1 FROM json_each(json_extract(e.meta, '$.tags')) WHERE value IN (");
    {
        let mut separated = query.separated(", ");
        for tag in tags {
            separated.push_bind(tag.clone());
        }
    }
    query.push("))");
}

fn push_exact_scope(
    query: &mut QueryBuilder<'_, Sqlite>,
    has_where: &mut bool,
    scope_path: &ScopePath,
) {
    push_where_prefix(query, has_where);
    query.push("scope_path = ");
    query.push_bind(scope_path.as_str().to_owned());
}

fn push_scope_set<'a>(
    query: &mut QueryBuilder<'_, Sqlite>,
    has_where: &mut bool,
    scope_paths: impl IntoIterator<Item = &'a str>,
) {
    let scope_paths: Vec<&str> = scope_paths.into_iter().collect();
    push_where_prefix(query, has_where);
    if scope_paths.is_empty() {
        query.push("0 = 1");
        return;
    }
    query.push("scope_path IN (");
    {
        let mut separated = query.separated(", ");
        for scope_path in scope_paths {
            separated.push_bind(scope_path.to_owned());
        }
    }
    query.push(")");
}

#[cfg(test)]
mod tests {
    use cm_core::ScopePath;
    use sqlx::{Row, sqlite::SqlitePoolOptions};

    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn subtree_scope_filter_uses_scope_index() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(":memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE entries (
                id TEXT PRIMARY KEY,
                scope_path TEXT NOT NULL,
                superseded_by TEXT,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("CREATE INDEX idx_entries_scope ON entries(scope_path)")
            .execute(&pool)
            .await
            .unwrap();

        let scope = ScopePath::parse("global/project:alpha").unwrap();
        let mut has_where = true;
        let mut query = QueryBuilder::new(
            "EXPLAIN QUERY PLAN SELECT id FROM entries WHERE superseded_by IS NULL",
        );
        push_scope_filter(&mut query, &mut has_where, &ScopeFilter::Subtree(scope));

        let rows = query.build().fetch_all(&pool).await.unwrap();
        let plan = rows
            .iter()
            .map(|row| row.get::<String, _>("detail"))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            plan.contains("idx_entries_scope"),
            "subtree predicate should use idx_entries_scope:\n{plan}"
        );
        assert!(
            !plan.contains("SCAN entries"),
            "subtree predicate should not full scan entries:\n{plan}"
        );
    }
}
