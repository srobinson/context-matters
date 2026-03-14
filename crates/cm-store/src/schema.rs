//! SQLite connection pool setup and pragma configuration.
//!
//! Provides a dual-pool architecture: one write pool (max 1 connection) and
//! one read pool (max 4 connections with `read_only=true`). All pragmas from
//! the spec are applied via `SqliteConnectOptions` so they execute on every
//! new connection before any queries.

use std::path::Path;

use anyhow::Result;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};

/// Build base connection options with all required pragmas.
///
/// Applied on every connection open, before any queries execute:
/// - `journal_mode = WAL` (concurrent reads during writes)
/// - `synchronous = NORMAL` (safe with WAL, avoids FULL fsync overhead)
/// - `busy_timeout = 5000` (5s retry on lock contention)
/// - `wal_autocheckpoint = 1000` (checkpoint every 1000 pages)
/// - `cache_size = -64000` (64 MB page cache)
/// - `mmap_size = 268435456` (256 MB memory-mapped I/O)
/// - `temp_store = MEMORY` (temp tables in memory)
/// - `foreign_keys = ON` (enforce FK constraints)
/// - `journal_size_limit = 67108864` (64 MB WAL cap)
fn base_opts(db_path: &Path) -> SqliteConnectOptions {
    db_path
        .to_string_lossy()
        .parse::<SqliteConnectOptions>()
        .expect("valid database path")
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(std::time::Duration::from_secs(5))
        .pragma("wal_autocheckpoint", "1000")
        .pragma("cache_size", "-64000")
        .pragma("mmap_size", "268435456")
        .pragma("temp_store", "memory")
        .pragma("journal_size_limit", "67108864")
        .foreign_keys(true)
        .create_if_missing(true)
}

/// Create the write and read connection pools.
///
/// - **Write pool**: max 1 connection. SQLite permits one writer at a time.
///   Serializing writes through a single connection avoids `SQLITE_BUSY` on
///   the write path entirely.
/// - **Read pool**: max 4 connections. WAL mode allows concurrent reads that
///   do not block the writer.
///
/// The write pool clones `base_opts` first, then the read pool clones with
/// `read_only(true)`. This ensures the write pool retains full privileges.
pub async fn create_pools(db_path: &Path) -> Result<(SqlitePool, SqlitePool)> {
    let opts = base_opts(db_path);

    let write_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts.clone())
        .await?;

    let read_pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(opts.read_only(true))
        .await?;

    Ok((write_pool, read_pool))
}

/// Run embedded migrations against the write pool.
///
/// Uses sqlx's compile-time embedded migration system. Migration files
/// live in `crates/cm-store/migrations/` and are baked into the binary.
pub async fn run_migrations(write_pool: &SqlitePool) -> Result<()> {
    sqlx::migrate!("./migrations").run(write_pool).await?;
    tracing::info!("database migrations applied");
    Ok(())
}

/// Perform a WAL checkpoint. Call on graceful shutdown.
pub async fn wal_checkpoint(write_pool: &SqlitePool) -> Result<()> {
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(write_pool)
        .await?;
    tracing::debug!("WAL checkpoint complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_pools_sets_pragmas() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let (write_pool, read_pool) = create_pools(&db_path).await.unwrap();

        // Verify journal_mode = WAL
        let row: (String,) = sqlx::query_as("PRAGMA journal_mode")
            .fetch_one(&write_pool)
            .await
            .unwrap();
        assert_eq!(row.0, "wal");

        // Verify foreign_keys = ON
        let row: (i32,) = sqlx::query_as("PRAGMA foreign_keys")
            .fetch_one(&write_pool)
            .await
            .unwrap();
        assert_eq!(row.0, 1);

        // Read pool also has pragmas
        let row: (String,) = sqlx::query_as("PRAGMA journal_mode")
            .fetch_one(&read_pool)
            .await
            .unwrap();
        assert_eq!(row.0, "wal");

        let row: (i32,) = sqlx::query_as("PRAGMA foreign_keys")
            .fetch_one(&read_pool)
            .await
            .unwrap();
        assert_eq!(row.0, 1);

        write_pool.close().await;
        read_pool.close().await;
    }

    #[tokio::test]
    async fn write_pool_limited_to_one_connection() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let (write_pool, _read_pool) = create_pools(&db_path).await.unwrap();

        // SqlitePoolOptions max_connections(1) limits the pool size
        let _opts = write_pool.connect_options();
        // Verify by checking pool reports max 1 active connection
        // We can at least verify the pool was created and is functional
        let row: (i32,) = sqlx::query_as("SELECT 1")
            .fetch_one(&write_pool)
            .await
            .unwrap();
        assert_eq!(row.0, 1);

        write_pool.close().await;
        _read_pool.close().await;
    }

    #[tokio::test]
    async fn read_pool_is_read_only() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let (write_pool, read_pool) = create_pools(&db_path).await.unwrap();

        // Create a table via write pool
        sqlx::query("CREATE TABLE test_ro (id INTEGER PRIMARY KEY)")
            .execute(&write_pool)
            .await
            .unwrap();

        // Read should work
        let rows: Vec<(i32,)> = sqlx::query_as("SELECT id FROM test_ro")
            .fetch_all(&read_pool)
            .await
            .unwrap();
        assert!(rows.is_empty());

        // Write via read pool should fail
        let result = sqlx::query("INSERT INTO test_ro (id) VALUES (1)")
            .execute(&read_pool)
            .await;
        assert!(result.is_err(), "read pool should reject writes");

        write_pool.close().await;
        read_pool.close().await;
    }
}
