//! In-memory cache for recently accessed links.

use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use mongodb::bson::DateTime;
use tokio::sync::RwLock;

use crate::model::LinkDocument;

/// Bounded cache of recently accessed links.
#[derive(Clone, Debug)]
pub struct AccessCache {
    capacity: usize,
    inner: Arc<RwLock<AccessCacheInner>>,
}

#[derive(Debug, Default)]
struct AccessCacheInner {
    entries: HashMap<String, CachedAccess>,
    recent: VecDeque<String>,
}

/// Link fields needed to serve a cached redirect.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CachedAccess {
    pub original_url: String,
    expires_at: Option<DateTime>,
}

impl AccessCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            inner: Arc::new(RwLock::new(AccessCacheInner::default())),
        }
    }

    pub async fn get(&self, hash: &str, now: DateTime) -> Option<CachedAccess> {
        if self.capacity == 0 {
            return None;
        }

        let mut inner = self.inner.write().await;
        let cached = inner.entries.get(hash).cloned()?;

        if cached.is_expired_at(now) {
            inner.remove(hash);
            return None;
        }

        inner.mark_recent(hash);
        Some(cached)
    }

    pub async fn remember(&self, document: &LinkDocument) {
        if self.capacity == 0 {
            return;
        }

        let mut inner = self.inner.write().await;
        inner.entries.insert(
            document.hash.clone(),
            CachedAccess {
                original_url: document.original_url.clone(),
                expires_at: document.expires_at,
            },
        );
        inner.mark_recent(&document.hash);
        inner.evict_to(self.capacity);
    }

    pub async fn invalidate(&self, hash: &str) {
        if self.capacity == 0 {
            return;
        }

        self.inner.write().await.remove(hash);
    }
}

impl CachedAccess {
    fn is_expired_at(&self, now: DateTime) -> bool {
        self.expires_at.is_some_and(|expires_at| expires_at <= now)
    }
}

impl AccessCacheInner {
    fn mark_recent(&mut self, hash: &str) {
        self.recent.retain(|cached_hash| cached_hash != hash);
        self.recent.push_back(hash.to_owned());
    }

    fn remove(&mut self, hash: &str) {
        self.entries.remove(hash);
        self.recent.retain(|cached_hash| cached_hash != hash);
    }

    fn evict_to(&mut self, capacity: usize) {
        while self.entries.len() > capacity {
            let Some(oldest_hash) = self.recent.pop_front() else {
                break;
            };
            self.entries.remove(&oldest_hash);
        }
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

    #[tokio::test]
    async fn get_should_return_none_when_cache_is_disabled() {
        let cache = AccessCache::new(0);
        cache
            .remember(&document("a", "https://example.com/a"))
            .await;

        let cached = cache.get("a", DateTime::from_millis(2)).await;

        assert!(cached.is_none());
    }

    #[tokio::test]
    async fn remember_should_evict_oldest_entry_when_capacity_is_exceeded() {
        let cache = AccessCache::new(1);
        cache
            .remember(&document("a", "https://example.com/a"))
            .await;
        cache
            .remember(&document("b", "https://example.com/b"))
            .await;

        let cached = cache.get("a", DateTime::from_millis(2)).await;

        assert!(cached.is_none());
    }

    #[tokio::test]
    async fn get_should_promote_entry_before_eviction() {
        let cache = AccessCache::new(2);
        cache
            .remember(&document("a", "https://example.com/a"))
            .await;
        cache
            .remember(&document("b", "https://example.com/b"))
            .await;
        let _ = cache.get("a", DateTime::from_millis(2)).await;
        cache
            .remember(&document("c", "https://example.com/c"))
            .await;

        let cached = cache.get("a", DateTime::from_millis(3)).await;

        assert_eq!(
            cached.map(|entry| entry.original_url),
            Some("https://example.com/a".to_owned())
        );
    }
}
