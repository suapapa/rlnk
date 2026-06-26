//! Error types shared by the application layers.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

/// Errors raised while loading runtime configuration.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("missing required environment variable {0}")]
    MissingEnvironment(&'static str),
    #[error("invalid environment variable {name}: {reason}")]
    InvalidEnvironment { name: &'static str, reason: String },
}

/// Errors raised while bootstrapping the process.
#[derive(Debug, thiserror::Error)]
pub enum BootstrapError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Mongo(#[from] mongodb::error::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Application errors returned by handlers and repository code.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("invalid authorization header")]
    Unauthorized,
    #[error("resource not found")]
    NotFound,
    #[error("invalid URL: {0}")]
    InvalidUrl(String),
    #[error("unsupported URL scheme: {0}")]
    UnsupportedUrlScheme(String),
    #[error("invalid TTL: {0}")]
    InvalidTtl(String),
    #[error("hash already exists")]
    HashAlreadyExists,
    #[error("failed to generate a unique hash after multiple attempts")]
    HashCollisionExhausted,
    #[error("database error: {0}")]
    Database(#[from] mongodb::error::Error),
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: &'static str,
    message: String,
}

impl AppError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::InvalidUrl(_) | Self::UnsupportedUrlScheme(_) | Self::InvalidTtl(_) => {
                StatusCode::BAD_REQUEST
            }
            Self::HashAlreadyExists | Self::HashCollisionExhausted | Self::Database(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    fn error_code(&self) -> &'static str {
        match self {
            Self::Unauthorized => "unauthorized",
            Self::NotFound => "not_found",
            Self::InvalidUrl(_) => "invalid_url",
            Self::UnsupportedUrlScheme(_) => "unsupported_url_scheme",
            Self::InvalidTtl(_) => "invalid_ttl",
            Self::HashAlreadyExists => "hash_already_exists",
            Self::HashCollisionExhausted => "hash_collision_exhausted",
            Self::Database(_) => "database_error",
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let payload = ErrorResponse {
            error: self.error_code(),
            message: self.to_string(),
        };

        (status, Json(payload)).into_response()
    }
}
