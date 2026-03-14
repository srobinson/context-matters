//! SQLite persistence layer for context-matters.
//!
//! Implements the `ContextStore` trait from `cm-core` using sqlx
//! with a dual-pool architecture (1 writer, 4 readers).

pub mod config;
pub mod project;
pub mod schema;

pub use config::{Config, load as load_config};
pub use project::{default_base_dir, ensure_data_dir};
pub use schema::{create_pools, run_migrations, wal_checkpoint};
