//! Domain and transport models.

use std::time::Duration;

use mongodb::bson::{DateTime, oid::ObjectId};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// Incoming request payload for `POST /gen`.
#[derive(Debug, Deserialize)]
pub struct CreateLinkRequest {
    pub url: String,
    #[serde(default)]
    pub ttl: Option<String>,
}

/// Successful response body for `POST /gen`.
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct CreateLinkResponse {
    pub hash: String,
    pub short_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

/// Link summary returned by `GET /stat`.
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct LinkStatsResponse {
    pub hash: String,
    pub short_url: String,
    pub original_url: String,
    pub access_count: u64,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_accessed_at: Option<String>,
}

/// Validated input for creating a new short link.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NewLink {
    pub original_url: String,
    pub expires_at: Option<DateTime>,
}

/// MongoDB document representation of a short link.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct LinkDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub hash: String,
    pub original_url: String,
    pub created_at: DateTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime>,
    pub access_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_accessed_at: Option<DateTime>,
}

impl CreateLinkRequest {
    pub fn validate(self, now: DateTime) -> Result<NewLink, AppError> {
        Ok(NewLink {
            original_url: normalize_url(&self.url)?,
            expires_at: parse_expiration(self.ttl.as_deref(), now)?,
        })
    }
}

impl LinkDocument {
    pub fn new(hash: String, new_link: &NewLink, created_at: DateTime) -> Self {
        Self {
            id: None,
            hash,
            original_url: new_link.original_url.clone(),
            created_at,
            expires_at: new_link.expires_at,
            access_count: 0,
            last_accessed_at: None,
        }
    }

    pub fn is_expired_at(&self, now: DateTime) -> bool {
        self.expires_at.is_some_and(|expires_at| expires_at <= now)
    }

    pub fn into_create_response(self, app_hostname: &str) -> CreateLinkResponse {
        CreateLinkResponse {
            short_url: build_short_url(app_hostname, &self.hash),
            hash: self.hash,
            expires_at: self.expires_at.map(format_datetime),
        }
    }

    pub fn into_stats_response(self, app_hostname: &str) -> LinkStatsResponse {
        LinkStatsResponse {
            short_url: build_short_url(app_hostname, &self.hash),
            hash: self.hash,
            original_url: self.original_url,
            access_count: self.access_count,
            created_at: format_datetime(self.created_at),
            expires_at: self.expires_at.map(format_datetime),
            last_accessed_at: self.last_accessed_at.map(format_datetime),
        }
    }
}

pub fn build_short_url(app_hostname: &str, hash: &str) -> String {
    format!("{app_hostname}/{hash}")
}

pub fn normalize_url(raw: &str) -> Result<String, AppError> {
    let parsed = url::Url::parse(raw).map_err(|source| AppError::InvalidUrl(source.to_string()))?;

    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::UnsupportedUrlScheme(parsed.scheme().to_owned()));
    }

    if parsed.host_str().is_none() {
        return Err(AppError::InvalidUrl("URL must include a host".to_owned()));
    }

    Ok(parsed.to_string())
}

pub fn parse_expiration(ttl: Option<&str>, now: DateTime) -> Result<Option<DateTime>, AppError> {
    ttl.map(parse_ttl_duration)
        .transpose()?
        .map(|duration| {
            now.to_system_time()
                .checked_add(duration)
                .map(DateTime::from_system_time)
                .ok_or_else(|| AppError::InvalidTtl("TTL is too large".to_owned()))
        })
        .transpose()
}

pub fn format_datetime(value: DateTime) -> String {
    humantime::format_rfc3339_seconds(value.to_system_time()).to_string()
}

fn parse_ttl_duration(raw: &str) -> Result<Duration, AppError> {
    let duration = humantime::parse_duration(raw)
        .map_err(|source| AppError::InvalidTtl(source.to_string()))?;

    if duration.is_zero() {
        return Err(AppError::InvalidTtl(
            "TTL must be greater than zero".to_owned(),
        ));
    }

    Ok(duration)
}

#[cfg(test)]
mod tests {
    use mongodb::bson::DateTime;

    use super::{CreateLinkRequest, build_short_url, normalize_url, parse_expiration};

    #[test]
    fn normalize_url_should_accept_http_and_https_urls() {
        let normalized =
            normalize_url("https://example.com/path?x=1").expect("URL should be valid");

        assert_eq!(normalized, "https://example.com/path?x=1");
    }

    #[test]
    fn normalize_url_should_reject_non_http_schemes() {
        let error = normalize_url("ftp://example.com").expect_err("ftp should be rejected");

        assert_eq!(error.to_string(), "unsupported URL scheme: ftp");
    }

    #[test]
    fn parse_expiration_should_return_none_when_ttl_is_missing() {
        let expires_at =
            parse_expiration(None, DateTime::now()).expect("missing TTL should succeed");

        assert!(expires_at.is_none());
    }

    #[test]
    fn parse_expiration_should_return_error_when_ttl_is_zero() {
        let error =
            parse_expiration(Some("0s"), DateTime::now()).expect_err("zero TTL should fail");

        assert_eq!(
            error.to_string(),
            "invalid TTL: TTL must be greater than zero"
        );
    }

    #[test]
    fn validate_should_accept_duration_strings() {
        let request = CreateLinkRequest {
            url: "https://example.com".to_owned(),
            ttl: Some("10m".to_owned()),
        };

        let new_link = request
            .validate(DateTime::now())
            .expect("request should validate");

        assert!(new_link.expires_at.is_some());
    }

    #[test]
    fn build_short_url_should_join_base_and_hash() {
        let short_url = build_short_url("https://rlnk.test", "abc123");

        assert_eq!(short_url, "https://rlnk.test/abc123");
    }
}
