//! In-memory cache for recently accessed links.

use moka::sync::Cache;
use mongodb::bson::DateTime;

use crate::model::LinkDocument;

/// Bounded cache of recently accessed links.
#[derive(Clone, Debug)]
pub struct AccessCache {
    inner: Option<Cache<String, CachedAccess>>,
}

/// Link fields needed to serve a cached redirect.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CachedAccess {
    pub original_url: String,
    expires_at: Option<DateTime>,
}

impl AccessCache {
    pub fn new(capacity: usize) -> Self {
        if capacity == 0 {
            return Self { inner: None };
        }

        Self {
            inner: Some(Cache::builder().max_capacity(capacity as u64).build()),
        }
    }

    pub fn get(&self, hash: &str, now: DateTime) -> Option<CachedAccess> {
        let cache = self.inner.as_ref()?;
        let cached = cache.get(hash)?;

        if cached.is_expired_at(now) {
            cache.invalidate(hash);
            return None;
        }

        Some(cached)
    }

    pub fn remember(&self, document: &LinkDocument) {
        let Some(cache) = self.inner.as_ref() else {
            return;
        };

        cache.insert(
            document.hash.clone(),
            CachedAccess {
                original_url: document.original_url.clone(),
                expires_at: document.expires_at,
            },
        );
    }

    pub fn invalidate(&self, hash: &str) {
        if let Some(cache) = self.inner.as_ref() {
            cache.invalidate(hash);
        }
    }
}

impl CachedAccess {
    fn is_expired_at(&self, now: DateTime) -> bool {
        self.expires_at.is_some_and(|expires_at| expires_at <= now)
    }
}

#[cfg(test)]
mod tests {
    use mongodb::bson::DateTime;

    use crate::model::{LinkDocument, NewLink};

    use super::AccessCache;

    fn document(hash: &str, original_url: &str) -> LinkDocument {
        LinkDocument::new(
            hash.to_owned(),
            &NewLink {
                original_url: original_url.to_owned(),
                expires_at: None,
            },
            DateTime::from_millis(1),
        )
    }

    #[test]
    fn get_should_return_none_when_cache_is_disabled() {
        let cache = AccessCache::new(0);
        cache.remember(&document("a", "https://example.com/a"));

        let cached = cache.get("a", DateTime::from_millis(2));

        assert!(cached.is_none());
    }

    #[test]
    fn remember_should_return_cached_url_until_invalidated() {
        let cache = AccessCache::new(8);
        cache.remember(&document("a", "https://example.com/a"));

        let cached = cache.get("a", DateTime::from_millis(2));
        assert_eq!(
            cached.map(|entry| entry.original_url),
            Some("https://example.com/a".to_owned())
        );

        cache.invalidate("a");
        assert!(cache.get("a", DateTime::from_millis(3)).is_none());
    }

    #[test]
    fn get_should_return_none_for_expired_entry() {
        let mut link = document("a", "https://example.com/a");
        link.expires_at = Some(DateTime::from_millis(5));
        let cache = AccessCache::new(8);
        cache.remember(&link);

        assert!(cache.get("a", DateTime::from_millis(5)).is_none());
        assert!(cache.get("a", DateTime::from_millis(6)).is_none());
    }
}
