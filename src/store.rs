//! Storage abstractions and implementations.

use std::{collections::HashMap, sync::Arc, time::Duration};

use async_trait::async_trait;
use futures_util::TryStreamExt;
use mongodb::{
    Client, Collection, IndexModel,
    bson::{Bson, DateTime, doc},
    error::{ErrorKind, WriteFailure},
    options::{ClientOptions, IndexOptions},
};
use tokio::sync::RwLock;

use crate::{
    config::AppConfig,
    error::AppError,
    model::{LinkDocument, NewLink},
};

const DUPLICATE_KEY_CODE: i32 = 11_000;

/// Repository operations required by the HTTP layer.
#[async_trait]
pub trait LinkStore: Clone + Send + Sync + 'static {
    async fn create_indexes(&self) -> Result<(), AppError>;
    async fn ping(&self) -> Result<(), AppError>;
    async fn insert_link(
        &self,
        hash: &str,
        new_link: &NewLink,
        created_at: DateTime,
    ) -> Result<LinkDocument, AppError>;
    async fn get_active_link(
        &self,
        hash: &str,
        now: DateTime,
    ) -> Result<Option<LinkDocument>, AppError>;
    async fn apply_access(
        &self,
        hash: &str,
        access_count: u64,
        accessed_at: DateTime,
    ) -> Result<bool, AppError>;
    async fn delete_link(&self, hash: &str) -> Result<bool, AppError>;
    async fn list_links(
        &self,
        now: DateTime,
        limit: usize,
        offset: u64,
    ) -> Result<(Vec<LinkDocument>, u64), AppError>;
}

/// MongoDB-backed link repository.
#[derive(Clone, Debug)]
pub struct MongoLinkStore {
    database: mongodb::Database,
    collection: Collection<LinkDocument>,
}

impl MongoLinkStore {
    pub async fn connect(config: &AppConfig) -> Result<Self, mongodb::error::Error> {
        let mut options = ClientOptions::parse(&config.mongo_uri).await?;
        if let Some(max_pool_size) = config.mongo_max_pool_size {
            options.max_pool_size = Some(max_pool_size);
        }
        if let Some(connect_timeout) = config.mongo_connect_timeout {
            options.connect_timeout = Some(connect_timeout);
        }
        if let Some(server_selection_timeout) = config.mongo_server_selection_timeout {
            options.server_selection_timeout = Some(server_selection_timeout);
        }

        let client = Client::with_options(options)?;
        let database = client.database(&config.mongo_database);
        let collection = database.collection::<LinkDocument>(&config.mongo_collection);

        Ok(Self {
            database,
            collection,
        })
    }
}

#[async_trait]
impl LinkStore for MongoLinkStore {
    async fn create_indexes(&self) -> Result<(), AppError> {
        let unique_hash_index = IndexModel::builder()
            .keys(doc! { "hash": 1 })
            .options(
                IndexOptions::builder()
                    .name(Some("hash_unique".to_owned()))
                    .unique(Some(true))
                    .build(),
            )
            .build();
        let ttl_index = IndexModel::builder()
            .keys(doc! { "expires_at": 1 })
            .options(
                IndexOptions::builder()
                    .name(Some("expires_at_ttl".to_owned()))
                    .expire_after(Some(Duration::ZERO))
                    .build(),
            )
            .build();

        self.collection
            .create_indexes([unique_hash_index, ttl_index])
            .await?;

        Ok(())
    }

    async fn ping(&self) -> Result<(), AppError> {
        self.database
            .run_command(doc! { "ping": 1 })
            .await
            .map_err(AppError::Database)?;
        Ok(())
    }

    async fn insert_link(
        &self,
        hash: &str,
        new_link: &NewLink,
        created_at: DateTime,
    ) -> Result<LinkDocument, AppError> {
        let document = LinkDocument::new(hash.to_owned(), new_link, created_at);

        match self.collection.insert_one(&document).await {
            Ok(_) => Ok(document),
            Err(error) if is_duplicate_key_error(&error) => Err(AppError::HashAlreadyExists),
            Err(error) => Err(AppError::Database(error)),
        }
    }

    async fn get_active_link(
        &self,
        hash: &str,
        now: DateTime,
    ) -> Result<Option<LinkDocument>, AppError> {
        self.collection
            .find_one(active_link_filter(hash, now))
            .await
            .map_err(AppError::Database)
    }

    async fn apply_access(
        &self,
        hash: &str,
        access_count: u64,
        accessed_at: DateTime,
    ) -> Result<bool, AppError> {
        if access_count == 0 {
            return Ok(false);
        }

        let result = self
            .collection
            .update_one(
                active_link_filter(hash, accessed_at),
                access_update(access_count, accessed_at),
            )
            .await
            .map_err(AppError::Database)?;

        Ok(result.matched_count > 0)
    }

    async fn delete_link(&self, hash: &str) -> Result<bool, AppError> {
        let result = self
            .collection
            .delete_one(doc! { "hash": hash })
            .await
            .map_err(AppError::Database)?;

        Ok(result.deleted_count > 0)
    }

    async fn list_links(
        &self,
        now: DateTime,
        limit: usize,
        offset: u64,
    ) -> Result<(Vec<LinkDocument>, u64), AppError> {
        let filter = active_links_filter(now);
        let total = self
            .collection
            .count_documents(filter.clone())
            .await
            .map_err(AppError::Database)?;

        let documents = self
            .collection
            .find(filter)
            .sort(doc! { "created_at": -1_i32, "hash": 1_i32 })
            .skip(offset)
            .limit(limit as i64)
            .await
            .map_err(AppError::Database)?
            .try_collect::<Vec<_>>()
            .await
            .map_err(AppError::Database)?;

        Ok((documents, total))
    }
}

/// In-memory repository used by tests and local handler validation.
#[derive(Clone, Debug, Default)]
pub struct MemoryLinkStore {
    links: Arc<RwLock<HashMap<String, LinkDocument>>>,
}

impl MemoryLinkStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl LinkStore for MemoryLinkStore {
    async fn create_indexes(&self) -> Result<(), AppError> {
        Ok(())
    }

    async fn ping(&self) -> Result<(), AppError> {
        Ok(())
    }

    async fn insert_link(
        &self,
        hash: &str,
        new_link: &NewLink,
        created_at: DateTime,
    ) -> Result<LinkDocument, AppError> {
        let mut links = self.links.write().await;
        if links.contains_key(hash) {
            return Err(AppError::HashAlreadyExists);
        }

        let document = LinkDocument::new(hash.to_owned(), new_link, created_at);
        links.insert(hash.to_owned(), document.clone());
        Ok(document)
    }

    async fn get_active_link(
        &self,
        hash: &str,
        now: DateTime,
    ) -> Result<Option<LinkDocument>, AppError> {
        let mut links = self.links.write().await;
        let Some(link) = links.get(hash) else {
            return Ok(None);
        };

        if link.is_expired_at(now) {
            links.remove(hash);
            return Ok(None);
        }

        Ok(Some(link.clone()))
    }

    async fn apply_access(
        &self,
        hash: &str,
        access_count: u64,
        accessed_at: DateTime,
    ) -> Result<bool, AppError> {
        if access_count == 0 {
            return Ok(false);
        }

        let mut links = self.links.write().await;
        let Some(link) = links.get_mut(hash) else {
            return Ok(false);
        };

        if link.is_expired_at(accessed_at) {
            links.remove(hash);
            return Ok(false);
        }

        link.access_count = link.access_count.saturating_add(access_count);
        link.last_accessed_at = Some(accessed_at);

        Ok(true)
    }

    async fn delete_link(&self, hash: &str) -> Result<bool, AppError> {
        let removed = self.links.write().await.remove(hash);

        Ok(removed.is_some())
    }

    async fn list_links(
        &self,
        now: DateTime,
        limit: usize,
        offset: u64,
    ) -> Result<(Vec<LinkDocument>, u64), AppError> {
        let mut links = self
            .links
            .read()
            .await
            .values()
            .filter(|link| !link.is_expired_at(now))
            .cloned()
            .collect::<Vec<_>>();

        let total = links.len() as u64;

        links.sort_by(|left, right| {
            right
                .created_at
                .timestamp_millis()
                .cmp(&left.created_at.timestamp_millis())
                .then_with(|| left.hash.cmp(&right.hash))
        });

        let start = usize::try_from(offset)
            .unwrap_or(usize::MAX)
            .min(links.len());
        let end = start.saturating_add(limit).min(links.len());
        let page = links[start..end].to_vec();

        Ok((page, total))
    }
}

fn active_links_filter(now: DateTime) -> mongodb::bson::Document {
    doc! {
        "$or": [
            { "expires_at": { "$exists": false } },
            { "expires_at": Bson::Null },
            { "expires_at": { "$gt": now } }
        ]
    }
}

fn active_link_filter(hash: &str, now: DateTime) -> mongodb::bson::Document {
    doc! {
        "hash": hash,
        "$or": [
            { "expires_at": { "$exists": false } },
            { "expires_at": Bson::Null },
            { "expires_at": { "$gt": now } }
        ]
    }
}

fn access_update(access_count: u64, accessed_at: DateTime) -> mongodb::bson::Document {
    doc! {
        "$inc": { "access_count": access_count as i64 },
        "$set": { "last_accessed_at": accessed_at }
    }
}

fn is_duplicate_key_error(error: &mongodb::error::Error) -> bool {
    matches!(
        error.kind.as_ref(),
        ErrorKind::Write(WriteFailure::WriteError(write_error))
            if write_error.code == DUPLICATE_KEY_CODE
    )
}
