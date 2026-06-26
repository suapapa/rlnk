//! Authorization helpers and extractors.

use axum::http::HeaderMap;

use crate::error::AppError;

/// Validate the configured admin authorization header.
pub fn authorize(headers: &HeaderMap, expected_key: &str) -> Result<(), AppError> {
    let header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());

    match header {
        Some(value) if is_matching_bearer_token(value, expected_key) => Ok(()),
        _ => Err(AppError::Unauthorized),
    }
}

fn is_matching_bearer_token(value: &str, expected_key: &str) -> bool {
    let Some((scheme, token)) = value.split_once(' ') else {
        return false;
    };

    scheme.eq_ignore_ascii_case("Bearer") && token == expected_key
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue, header::AUTHORIZATION};

    use super::authorize;

    #[test]
    fn authorize_should_accept_matching_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer secret"));

        let result = authorize(&headers, "secret");

        assert!(result.is_ok());
    }

    #[test]
    fn authorize_should_accept_case_insensitive_bearer_scheme() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("bearer secret"));

        let result = authorize(&headers, "secret");

        assert!(result.is_ok());
    }

    #[test]
    fn authorize_should_reject_raw_key_header() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("secret"));

        let result = authorize(&headers, "secret");

        assert_eq!(
            result.expect_err("raw key should fail").to_string(),
            "invalid authorization header"
        );
    }

    #[test]
    fn authorize_should_reject_wrong_authorization_scheme() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Basic secret"));

        let result = authorize(&headers, "secret");

        assert_eq!(
            result.expect_err("wrong scheme should fail").to_string(),
            "invalid authorization header"
        );
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
