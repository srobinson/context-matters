use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::update::UpdateResult;

/// Shared JSON projection for `cm update --json`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WebUpdateView {
    pub updated: String,
    pub content_hash: String,
}

pub fn project_web_update(result: &UpdateResult) -> WebUpdateView {
    WebUpdateView {
        updated: result.updated_id.clone(),
        content_hash: result.content_hash.clone(),
    }
}
