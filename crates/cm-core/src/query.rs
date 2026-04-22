//! Query helpers for structured and full text entry search.

mod builder;
mod fts;

pub use builder::QueryBuilder;
pub use fts::FtsQuery;

#[cfg(test)]
mod builder_tests;

#[cfg(test)]
mod fts_tests;
