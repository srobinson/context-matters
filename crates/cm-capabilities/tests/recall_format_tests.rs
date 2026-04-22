//! Snapshot tests for `cm_capabilities::projection::format_recall_view`.
//!
//! Builds `RecallResult` fixtures covering the routing branches that
//! materially change the rendered shape:
//!
//!   * `Search` with populated BM25 scores; exercises the score column
//!     and the FTS5 routing advisory.
//!   * `BrowseFallback` without scores; exercises the no score row
//!     shape and the browse fallback advisory.
//!   * Empty result for any routing; exercises the `no matches` trailer
//!     and verifies the header still renders.
//!
//! Each test renders via [`format_recall_view_at`] with a pinned `now`
//! and diffs byte for byte against the golden on disk. Any intentional
//! wire shape change must update the golden.
//!
//! No SQLite store is involved. The formatter is pure (`RecallResult`
//! in, `String` out) so every fixture is built inline.

mod recall_format;
