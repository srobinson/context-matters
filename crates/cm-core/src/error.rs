use thiserror::Error;
use uuid::Uuid;

/// Errors specific to scope path parsing and validation.
///
/// Separated from `CmError` because scope path validation is a pure
/// operation (no I/O) and can be tested independently of the store.
#[derive(Debug, Error)]
pub enum ScopePathError {
    /// The input string is empty.
    #[error("scope path cannot be empty")]
    Empty,

    /// The path exceeds the maximum length.
    #[error("scope path too long: {len} bytes (max {max})")]
    TooLong { len: usize, max: usize },

    /// The path does not start with "global".
    #[error("scope path must start with 'global'")]
    MissingGlobalRoot,

    /// A segment does not follow the `kind:identifier` format.
    #[error("malformed segment: '{0}' (expected 'kind:identifier')")]
    MalformedSegment(String),

    /// A scope kind is not recognized.
    #[error("invalid scope kind: '{0}' (expected global, project, repo, or session)")]
    InvalidKind(String),

    /// Scope kinds are not in ascending hierarchical order.
    #[error("scope kind '{got}' cannot appear after '{after}'")]
    OutOfOrder { got: String, after: String },

    /// An identifier contains invalid characters.
    #[error("invalid identifier: '{0}' (must match [a-z0-9][a-z0-9-]*[a-z0-9])")]
    InvalidIdentifier(String),
}

/// Top-level error type for context-matters operations.
///
/// All `ContextStore` methods return `Result<T, CmError>`.
/// Each variant maps to a distinct failure mode with enough
/// context for the caller to construct a meaningful error response.
#[derive(Debug, Error)]
pub enum CmError {
    /// The requested entry does not exist.
    #[error("entry not found: {0}")]
    EntryNotFound(Uuid),

    /// The requested scope does not exist.
    #[error("scope not found: {0}")]
    ScopeNotFound(String),

    /// An entry with the same content hash already exists (active, not superseded).
    /// The `Uuid` is the ID of the existing duplicate.
    #[error("duplicate content: existing entry {0}")]
    DuplicateContent(Uuid),

    /// Scope path validation failed.
    #[error("invalid scope path: {0}")]
    InvalidScopePath(#[from] ScopePathError),

    /// An invalid entry kind string was provided.
    #[error("invalid entry kind: {0}")]
    InvalidEntryKind(String),

    /// An invalid relation kind string was provided.
    #[error("invalid relation kind: {0}")]
    InvalidRelationKind(String),

    /// A field validation check failed.
    #[error("validation error: {0}")]
    Validation(String),

    /// Foreign key constraint violation (e.g., scope has entries, cannot delete).
    #[error("constraint violation: {0}")]
    ConstraintViolation(String),

    /// JSON serialization or deserialization failed.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// Database error from the storage backend.
    #[error("database error: {0}")]
    Database(String),

    /// An internal error that should not occur during normal operation.
    #[error("internal error: {0}")]
    Internal(String),
}
