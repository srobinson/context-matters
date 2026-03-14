//! Tool handler stubs for the 9 `cx_*` tools.
//!
//! Each tool will be implemented in its own issue. For now, all handlers
//! return a placeholder indicating the tool is not yet implemented.

use cm_store::CmStore;
use serde_json::Value;

pub fn cx_recall(_store: &CmStore, _args: &Value) -> Result<String, String> {
    Err("cx_recall not yet implemented".to_owned())
}

pub fn cx_store(_store: &CmStore, _args: &Value) -> Result<String, String> {
    Err("cx_store not yet implemented".to_owned())
}

pub fn cx_deposit(_store: &CmStore, _args: &Value) -> Result<String, String> {
    Err("cx_deposit not yet implemented".to_owned())
}

pub fn cx_browse(_store: &CmStore, _args: &Value) -> Result<String, String> {
    Err("cx_browse not yet implemented".to_owned())
}

pub fn cx_get(_store: &CmStore, _args: &Value) -> Result<String, String> {
    Err("cx_get not yet implemented".to_owned())
}

pub fn cx_update(_store: &CmStore, _args: &Value) -> Result<String, String> {
    Err("cx_update not yet implemented".to_owned())
}

pub fn cx_forget(_store: &CmStore, _args: &Value) -> Result<String, String> {
    Err("cx_forget not yet implemented".to_owned())
}

pub fn cx_stats(_store: &CmStore, _args: &Value) -> Result<String, String> {
    Err("cx_stats not yet implemented".to_owned())
}

pub fn cx_export(_store: &CmStore, _args: &Value) -> Result<String, String> {
    Err("cx_export not yet implemented".to_owned())
}
