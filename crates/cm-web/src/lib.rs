pub mod api;

use cm_store::CmStore;

/// Shared application state passed to all handlers.
pub struct AppState {
    pub store: CmStore,
}
