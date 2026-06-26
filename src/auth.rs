//! Authorization helpers and extractors.

use axum::http::HeaderMap;

use crate::error::AppError;

/// Validate the configured admin authorization header.
pub fn authorize(headers: &HeaderMap, expected_key: &str) -> Result<(), AppError> {
    let header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());

    match header {
        Some(value) if value == expected_key => Ok(()),
        _ => Err(AppError::Unauthorized),
    }
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue, header::AUTHORIZATION};

    use super::authorize;

    #[test]
    fn authorize_should_accept_matching_header_value() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("secret"));

        let result = authorize(&headers, "secret");

        assert!(result.is_ok());
    }

    #[test]
    fn authorize_should_reject_missing_header() {
        let result = authorize(&HeaderMap::new(), "secret");

        assert_eq!(
            result.expect_err("missing header should fail").to_string(),
            "invalid authorization header"
        );
    }
}
