//! SQLite persistence layer for context-matters.
//!
//! Implements the `ContextStore` trait from `cm-core` using sqlx
//! with a dual-pool architecture (1 writer, 4 readers).
