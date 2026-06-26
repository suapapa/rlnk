//! Application configuration loading and validation.

use std::{collections::HashMap, net::SocketAddr, num::NonZeroUsize};

use crate::error::ConfigError;

const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8080";
const DEFAULT_DATABASE: &str = "rlnk";
const DEFAULT_COLLECTION: &str = "links";
const DEFAULT_HASH_LENGTH: usize = 8;
const DEFAULT_ACCESS_CACHE_SIZE: usize = 1024;

/// Runtime configuration loaded from environment variables.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppConfig {
    pub mongo_uri: String,
    pub app_key: String,
    pub app_hostname: String,
    pub bind_addr: SocketAddr,
    pub mongo_database: String,
    pub mongo_collection: String,
    pub hash_length: NonZeroUsize,
    pub access_cache_size: usize,
}

impl AppConfig {
    /// Load configuration from process environment variables.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_pairs(std::env::vars())
    }

    /// Load configuration from arbitrary key/value pairs.
    pub fn from_pairs<I, K, V>(vars: I) -> Result<Self, ConfigError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let vars = vars
            .into_iter()
            .map(|(key, value)| (key.into(), value.into()))
            .collect::<HashMap<_, _>>();

        let mongo_uri = required(&vars, "MONGO_URI")?;
        let app_key = required(&vars, "APP_KEY")?;
        let app_hostname = normalize_hostname(required(&vars, "APP_HOSTNAME")?)?;
        let bind_addr = optional(&vars, "APP_BIND_ADDR", DEFAULT_BIND_ADDR)
            .parse::<SocketAddr>()
            .map_err(|source| ConfigError::InvalidEnvironment {
                name: "APP_BIND_ADDR",
                reason: source.to_string(),
            })?;
        let mongo_database = optional(&vars, "MONGO_DATABASE", DEFAULT_DATABASE);
        let mongo_collection = optional(&vars, "MONGO_COLLECTION", DEFAULT_COLLECTION);
        let hash_length = match vars.get("HASH_LENGTH") {
            Some(raw) => parse_hash_length(raw)?,
            None => default_hash_length()?,
        };
        let access_cache_size = match vars.get("ACCESS_CACHE_SIZE") {
            Some(raw) => parse_access_cache_size(raw)?,
            None => DEFAULT_ACCESS_CACHE_SIZE,
        };

        Ok(Self {
            mongo_uri,
            app_key,
            app_hostname,
            bind_addr,
            mongo_database,
            mongo_collection,
            hash_length,
            access_cache_size,
        })
    }
}

fn required(vars: &HashMap<String, String>, name: &'static str) -> Result<String, ConfigError> {
    vars.get(name)
        .cloned()
        .filter(|value| !value.trim().is_empty())
        .ok_or(ConfigError::MissingEnvironment(name))
}

fn optional(vars: &HashMap<String, String>, name: &str, default: &str) -> String {
    vars.get(name)
        .cloned()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_owned())
}

fn normalize_hostname(raw: String) -> Result<String, ConfigError> {
    let trimmed = raw.trim().trim_end_matches('/').to_owned();
    let parsed = url::Url::parse(&trimmed).map_err(|source| ConfigError::InvalidEnvironment {
        name: "APP_HOSTNAME",
        reason: source.to_string(),
    })?;

    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(ConfigError::InvalidEnvironment {
            name: "APP_HOSTNAME",
            reason: "must use http or https".to_owned(),
        });
    }

    if parsed.host_str().is_none() {
        return Err(ConfigError::InvalidEnvironment {
            name: "APP_HOSTNAME",
            reason: "must include a host".to_owned(),
        });
    }

    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(ConfigError::InvalidEnvironment {
            name: "APP_HOSTNAME",
            reason: "must not include a query or fragment".to_owned(),
        });
    }

    Ok(trimmed)
}

fn parse_hash_length(raw: &str) -> Result<NonZeroUsize, ConfigError> {
    let parsed = raw
        .parse::<usize>()
        .map_err(|source| ConfigError::InvalidEnvironment {
            name: "HASH_LENGTH",
            reason: source.to_string(),
        })?;

    NonZeroUsize::new(parsed).ok_or(ConfigError::InvalidEnvironment {
        name: "HASH_LENGTH",
        reason: "must be greater than zero".to_owned(),
    })
}

fn default_hash_length() -> Result<NonZeroUsize, ConfigError> {
    NonZeroUsize::new(DEFAULT_HASH_LENGTH).ok_or(ConfigError::InvalidEnvironment {
        name: "HASH_LENGTH",
        reason: "default must be greater than zero".to_owned(),
    })
}

fn parse_access_cache_size(raw: &str) -> Result<usize, ConfigError> {
    raw.parse::<usize>()
        .map_err(|source| ConfigError::InvalidEnvironment {
            name: "ACCESS_CACHE_SIZE",
            reason: source.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    fn base_env() -> Vec<(&'static str, &'static str)> {
        vec![
            ("MONGO_URI", "mongodb://localhost:27017"),
            ("APP_KEY", "secret"),
            ("APP_HOSTNAME", "https://rlnk.test"),
        ]
    }

    #[test]
    fn from_pairs_should_load_defaults_when_optional_values_are_missing() {
        let config = AppConfig::from_pairs(base_env()).expect("config should load");

        assert_eq!(config.mongo_database, "rlnk");
        assert_eq!(config.mongo_collection, "links");
        assert_eq!(config.hash_length.get(), 8);
        assert_eq!(config.access_cache_size, 1024);
    }

    #[test]
    fn from_pairs_should_allow_disabling_access_cache() {
        let config = AppConfig::from_pairs([
            ("MONGO_URI", "mongodb://localhost:27017"),
            ("APP_KEY", "secret"),
            ("APP_HOSTNAME", "https://rlnk.test"),
            ("ACCESS_CACHE_SIZE", "0"),
        ])
        .expect("config should load");

        assert_eq!(config.access_cache_size, 0);
    }

    #[test]
    fn from_pairs_should_trim_trailing_slash_from_app_hostname() {
        let config = AppConfig::from_pairs([
            ("MONGO_URI", "mongodb://localhost:27017"),
            ("APP_KEY", "secret"),
            ("APP_HOSTNAME", "https://rlnk.test/"),
        ])
        .expect("config should load");

        assert_eq!(config.app_hostname, "https://rlnk.test");
    }

    #[test]
    fn from_pairs_should_return_error_when_required_variable_is_missing() {
        let error =
            AppConfig::from_pairs([("APP_KEY", "secret"), ("APP_HOSTNAME", "https://rlnk.test")])
                .expect_err("missing MONGO_URI should fail");

        assert_eq!(
            error.to_string(),
            "missing required environment variable MONGO_URI"
        );
    }

    #[test]
    fn from_pairs_should_return_error_when_hash_length_is_zero() {
        let error = AppConfig::from_pairs([
            ("MONGO_URI", "mongodb://localhost:27017"),
            ("APP_KEY", "secret"),
            ("APP_HOSTNAME", "https://rlnk.test"),
            ("HASH_LENGTH", "0"),
        ])
        .expect_err("zero hash length should fail");

        assert_eq!(
            error.to_string(),
            "invalid environment variable HASH_LENGTH: must be greater than zero"
        );
    }
}
