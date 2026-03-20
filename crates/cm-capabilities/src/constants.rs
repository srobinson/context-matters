/// Maximum input size for text-accepting tool fields (1 MB).
pub const MAX_INPUT_BYTES: usize = 1_048_576;

/// Maximum number of IDs in a batch request.
pub const MAX_BATCH_IDS: usize = 100;

/// Default result limit for retrieval tools.
pub const DEFAULT_LIMIT: u32 = 20;

/// Maximum result limit.
pub const MAX_LIMIT: u32 = 200;

/// Snippet length for two-phase retrieval responses.
pub const SNIPPET_LENGTH: usize = 200;
