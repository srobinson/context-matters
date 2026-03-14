//! Domain types and traits for the context-matters store.
//!
//! This crate defines the core abstractions with zero I/O dependencies.
//! The `ContextStore` trait uses synchronous method signatures.
//! Storage adapters (cm-store) wrap these in async where needed.
