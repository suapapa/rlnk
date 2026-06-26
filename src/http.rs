//! HTTP router and handlers.

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Redirect,
    routing::{get, post},
};
use mongodb::bson::DateTime;
use tower_http::trace::TraceLayer;

use crate::{
    auth::authorize,
    cache::AccessCache,
    config::AppConfig,
    error::AppError,
    hash::HashGenerator,
    model::{CreateLinkRequest, CreateLinkResponse, LinkDocument, LinkStatsResponse},
    store::LinkStore,
};

const MAX_HASH_GENERATION_ATTEMPTS: usize = 8;

/// Shared application state injected into handlers.
#[derive(Clone)]
pub struct AppState<R> {
    config: Arc<AppConfig>,
    store: R,
    hash_generator: HashGenerator,
    access_cache: AccessCache,
}

impl<R> AppState<R> {
    pub fn new(config: Arc<AppConfig>, store: R) -> Self {
        let hash_generator = HashGenerator::new(config.hash_length);
        let access_cache = AccessCache::new(config.access_cache_size);

        Self {
            config,
            store,
            hash_generator,
            access_cache,
        }
    }
}

/// Build the service router for the provided repository implementation.
pub fn app<R>(state: AppState<R>) -> Router
where
    R: LinkStore,
{
    Router::new()
        .route("/stat", get(list_links::<R>))
        .route("/gen", post(create_link::<R>))
        .route(
            "/{hash}",
            get(redirect_to_link::<R>).delete(delete_link::<R>),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn create_link<R>(
    State(state): State<AppState<R>>,
    headers: HeaderMap,
    Json(request): Json<CreateLinkRequest>,
) -> Result<Json<CreateLinkResponse>, AppError>
where
    R: LinkStore,
{
    authorize(&headers, &state.config.app_key)?;

    let now = DateTime::now();
    let new_link = request.validate(now)?;

    for _ in 0..MAX_HASH_GENERATION_ATTEMPTS {
        let hash = state.hash_generator.generate();
        match state.store.insert_link(&hash, &new_link, now).await {
            Ok(document) => {
                return Ok(Json(
                    document.into_create_response(&state.config.app_hostname),
                ));
            }
            Err(AppError::HashAlreadyExists) => {}
            Err(error) => return Err(error),
        }
    }

    Err(AppError::HashCollisionExhausted)
}

async fn delete_link<R>(
    State(state): State<AppState<R>>,
    headers: HeaderMap,
    Path(hash): Path<String>,
) -> Result<StatusCode, AppError>
where
    R: LinkStore,
{
    authorize(&headers, &state.config.app_key)?;

    let deleted = state.store.delete_link(&hash).await?;
    state.access_cache.invalidate(&hash).await;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}

async fn redirect_to_link<R>(
    State(state): State<AppState<R>>,
    Path(hash): Path<String>,
) -> Result<Redirect, AppError>
where
    R: LinkStore,
{
    let accessed_at = DateTime::now();
    if let Some(cached) = state.access_cache.get(&hash, accessed_at).await {
        if state.store.record_access(&hash, accessed_at).await? {
            return Ok(Redirect::temporary(&cached.original_url));
        }

        state.access_cache.invalidate(&hash).await;
        return Err(AppError::NotFound);
    }

    let Some(link) = state.store.touch_link(&hash, accessed_at).await? else {
        return Err(AppError::NotFound);
    };

    state.access_cache.remember(&link).await;
    Ok(Redirect::temporary(&link.original_url))
}

async fn list_links<R>(
    State(state): State<AppState<R>>,
    headers: HeaderMap,
) -> Result<Json<Vec<LinkStatsResponse>>, AppError>
where
    R: LinkStore,
{
    authorize(&headers, &state.config.app_key)?;

    let documents = state.store.list_links(DateTime::now()).await?;
    Ok(Json(to_stats_responses(
        documents,
        &state.config.app_hostname,
    )))
}

fn to_stats_responses(documents: Vec<LinkDocument>, app_hostname: &str) -> Vec<LinkStatsResponse> {
    documents
        .into_iter()
        .map(|document| document.into_stats_response(app_hostname))
        .collect()
}
