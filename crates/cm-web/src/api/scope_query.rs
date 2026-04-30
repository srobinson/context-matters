//! Shared query-string parsing for structured scope selectors.

use cm_capabilities::scope::ScopeSelector;
use cm_capabilities::validation::check_input_size;
use url::form_urlencoded;

use crate::api::error::ApiError;

pub(crate) fn err_scope_path_removed() -> ApiError {
    ApiError(cm_core::CmError::Validation(
        "use 'scope' instead of 'scope_path'".to_owned(),
    ))
}

pub(crate) fn err_scope_mode_removed() -> ApiError {
    ApiError(cm_core::CmError::Validation(
        "use 'scope' instead of 'scope_mode'".to_owned(),
    ))
}

pub(crate) fn err_cwd_removed() -> ApiError {
    ApiError(cm_core::CmError::Validation(
        "use scope kind 'cwd_inferred' with its cwd field instead of top-level 'cwd'".to_owned(),
    ))
}

pub(crate) fn err_unknown_query_key(key: &str, allowed: &[&str]) -> ApiError {
    ApiError(cm_core::CmError::Validation(format!(
        "unknown query parameter '{key}' (allowed: {})",
        allowed.join(", ")
    )))
}

pub(crate) fn parse_scope_value(scope: String) -> Result<ScopeSelector, ApiError> {
    check_input_size(&scope, "scope").map_err(|msg| ApiError(cm_core::CmError::Validation(msg)))?;
    ScopeSelector::parse(&scope).map_err(ApiError)
}

pub(crate) fn parse_optional_scope(
    scope: Option<String>,
) -> Result<Option<ScopeSelector>, ApiError> {
    scope.map(parse_scope_value).transpose()
}

const SCOPE_QUERY_KEYS: &[&str] = &["scope"];

pub(crate) fn parse_scope_query(raw: Option<&str>) -> Result<Option<ScopeSelector>, ApiError> {
    let mut scope = None;

    for (key, value) in form_urlencoded::parse(raw.unwrap_or_default().as_bytes()) {
        match key.as_ref() {
            "scope" => scope = Some(parse_scope_value(value.into_owned())?),
            "cwd" => return Err(err_cwd_removed()),
            "scope_path" => return Err(err_scope_path_removed()),
            "scope_mode" => return Err(err_scope_mode_removed()),
            other => return Err(err_unknown_query_key(other, SCOPE_QUERY_KEYS)),
        }
    }

    Ok(scope)
}

#[derive(Debug)]
pub(crate) struct SearchQuery {
    pub q: String,
    pub scope: Option<ScopeSelector>,
    pub kind: Option<String>,
    pub tag: Option<String>,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

const SEARCH_QUERY_KEYS: &[&str] = &["query", "scope", "kind", "tag", "limit", "cursor"];

pub(crate) fn parse_search_query(raw: Option<&str>) -> Result<SearchQuery, ApiError> {
    let mut q = None;
    let mut scope = None;
    let mut kind = None;
    let mut tag = None;
    let mut limit = None;
    let mut cursor = None;

    for (key, value) in form_urlencoded::parse(raw.unwrap_or_default().as_bytes()) {
        match key.as_ref() {
            "query" => q = Some(value.into_owned()),
            "scope" => scope = Some(parse_scope_value(value.into_owned())?),
            "cwd" => return Err(err_cwd_removed()),
            "scope_path" => return Err(err_scope_path_removed()),
            "scope_mode" => return Err(err_scope_mode_removed()),
            "kind" => kind = Some(value.into_owned()),
            "tag" => tag = Some(value.into_owned()),
            "limit" => {
                limit = Some(value.parse::<u32>().map_err(|_| {
                    ApiError(cm_core::CmError::Validation(format!(
                        "invalid limit: '{value}'"
                    )))
                })?)
            }
            "cursor" => cursor = Some(value.into_owned()),
            other => return Err(err_unknown_query_key(other, SEARCH_QUERY_KEYS)),
        }
    }

    Ok(SearchQuery {
        q: q.unwrap_or_default(),
        scope,
        kind,
        tag,
        limit,
        cursor,
    })
}
