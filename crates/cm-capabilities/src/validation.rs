use cm_core::Confidence;

use crate::constants::{DEFAULT_LIMIT, MAX_INPUT_BYTES, MAX_LIMIT};

/// Reject input exceeding the per-field byte limit.
pub fn check_input_size(value: &str, field: &str) -> Result<(), String> {
    if value.len() > MAX_INPUT_BYTES {
        return Err(format!("{field} exceeds {MAX_INPUT_BYTES} byte limit"));
    }
    Ok(())
}

/// Clamp a limit value to the allowed range `[1, MAX_LIMIT]`.
pub fn clamp_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
}

/// Parse a confidence string to the Confidence enum.
pub fn parse_confidence(s: &str) -> Result<Confidence, String> {
    match s {
        "high" => Ok(Confidence::High),
        "medium" => Ok(Confidence::Medium),
        "low" => Ok(Confidence::Low),
        other => Err(format!(
            "Invalid confidence '{other}'. Valid values: high, medium, low."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_limit_defaults_to_20() {
        assert_eq!(clamp_limit(None), DEFAULT_LIMIT);
    }

    #[test]
    fn clamp_limit_caps_at_max() {
        assert_eq!(clamp_limit(Some(500)), MAX_LIMIT);
    }

    #[test]
    fn clamp_limit_floors_at_1() {
        assert_eq!(clamp_limit(Some(0)), 1);
    }

    #[test]
    fn clamp_limit_passes_through_valid() {
        assert_eq!(clamp_limit(Some(50)), 50);
    }

    #[test]
    fn check_input_size_accepts_small() {
        assert!(check_input_size("hello", "field").is_ok());
    }

    #[test]
    fn check_input_size_rejects_large() {
        let big = "x".repeat(MAX_INPUT_BYTES + 1);
        assert!(check_input_size(&big, "body").is_err());
    }

    #[test]
    fn parse_confidence_valid() {
        assert_eq!(parse_confidence("high").unwrap(), Confidence::High);
        assert_eq!(parse_confidence("medium").unwrap(), Confidence::Medium);
        assert_eq!(parse_confidence("low").unwrap(), Confidence::Low);
    }

    #[test]
    fn parse_confidence_invalid() {
        assert!(parse_confidence("unknown").is_err());
    }
}
