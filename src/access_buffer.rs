//! Write-behind buffer for redirect access statistics.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use mongodb::bson::DateTime;
use tokio::{
    sync::watch,
    time::{MissedTickBehavior, interval},
};
use tracing::warn;

use crate::{error::AppError, store::LinkStore};

#[derive(Debug)]
struct PendingAccess {
    count: u64,
    last_accessed_at: DateTime,
}

/// Aggregates redirect access events and flushes them to the store in batches.
#[derive(Clone, Debug, Default)]
pub struct AccessBuffer {
    pending: Arc<Mutex<HashMap<String, PendingAccess>>>,
}

impl AccessBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one access for `hash`. Safe to call on the redirect hot path.
    pub fn record(&self, hash: impl Into<String>, accessed_at: DateTime) {
        let mut pending = match self.pending.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let entry = pending.entry(hash.into()).or_insert(PendingAccess {
            count: 0,
            last_accessed_at: accessed_at,
        });
        entry.count = entry.count.saturating_add(1);
        entry.last_accessed_at = accessed_at;
    }

    /// Drop buffered updates for a hash (for example after delete).
    pub fn discard(&self, hash: &str) {
        let mut pending = match self.pending.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        pending.remove(hash);
    }

    /// Flush all pending access updates to `store`.
    pub async fn flush<R>(&self, store: &R) -> Result<u64, AppError>
    where
        R: LinkStore,
    {
        let batch = {
            let mut pending = match self.pending.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            std::mem::take(&mut *pending)
        };

        let mut flushed = 0_u64;
        for (hash, access) in batch {
            if store
                .apply_access(&hash, access.count, access.last_accessed_at)
                .await?
            {
                flushed = flushed.saturating_add(1);
            }
        }

        Ok(flushed)
    }
}

/// Periodically flush buffered access stats until `shutdown` becomes true.
pub async fn run_access_flush_loop<R>(
    store: R,
    buffer: AccessBuffer,
    flush_interval: Duration,
    mut shutdown: watch::Receiver<bool>,
) where
    R: LinkStore,
{
    let mut ticker = interval(flush_interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if let Err(error) = buffer.flush(&store).await {
                    warn!(%error, "access stats flush failed");
                }
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    if let Err(error) = buffer.flush(&store).await {
                        warn!(%error, "final access stats flush failed");
                    }
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use mongodb::bson::DateTime;

    use crate::{
        model::NewLink,
        store::{LinkStore, MemoryLinkStore},
    };

    use super::AccessBuffer;

    #[tokio::test]
    async fn flush_should_apply_aggregated_access_counts() {
        let store = MemoryLinkStore::new();
        let created_at = DateTime::from_millis(1);
        store
            .insert_link(
                "abc",
                &NewLink {
                    original_url: "https://example.com".to_owned(),
                    expires_at: None,
                },
                created_at,
            )
            .await
            .expect("insert should succeed");

        let buffer = AccessBuffer::new();
        buffer.record("abc", DateTime::from_millis(2));
        buffer.record("abc", DateTime::from_millis(3));

        let flushed = buffer.flush(&store).await.expect("flush should succeed");
        assert_eq!(flushed, 1);

        let (links, _) = store
            .list_links(DateTime::from_millis(4), 10, 0)
            .await
            .expect("list should succeed");
        assert_eq!(links[0].access_count, 2);
        assert_eq!(links[0].last_accessed_at, Some(DateTime::from_millis(3)));
    }

    #[tokio::test]
    async fn discard_should_drop_pending_updates() {
        let store = MemoryLinkStore::new();
        let buffer = AccessBuffer::new();
        buffer.record("abc", DateTime::from_millis(2));
        buffer.discard("abc");

        let flushed = buffer.flush(&store).await.expect("flush should succeed");
        assert_eq!(flushed, 0);
    }
}
