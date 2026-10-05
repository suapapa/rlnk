use std::sync::{Arc, atomic::Ordering};

use axum::{
    Json, Router,
    extract::{Path, Query, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use mongodb::bson::DateTime;
use serde::Serialize;
use tower_http::trace::TraceLayer;

use crate::{
    access_buffer::AccessBuffer,
    auth::authorize,
    cache::AccessCache,
    config::AppConfig,
    error::AppError,
    hash::HashGenerator,
    metrics::AppMetrics,
    model::{
        CreateLinkRequest, CreateLinkResponse, LinkDocument, LinkStatsResponse,
        PaginatedLinkStatsResponse, StatQueryParams,
    },
    store::LinkStore,
};

const MAX_HASH_GENERATION_ATTEMPTS: usize = 8;
const PROMETHEUS_CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";

/// Shared application state injected into handlers.
#[derive(Clone)]
pub struct AppState<R> {
    config: Arc<AppConfig>,
    store: R,
    hash_generator: HashGenerator,
    access_cache: AccessCache,
    access_buffer: AccessBuffer,
    metrics: Arc<AppMetrics>,
}

impl<R> AppState<R>
where
    R: LinkStore,
{
    pub fn new(config: Arc<AppConfig>, store: R) -> Self {
        let hash_generator = HashGenerator::new(config.hash_length);
        let access_cache = AccessCache::new(config.access_cache_size);
        let access_buffer = AccessBuffer::new();
        let metrics = Arc::new(AppMetrics::default());

        Self {
            config,
            store,
            hash_generator,
            access_cache,
            access_buffer,
            metrics,
        }
    }

    pub fn metrics(&self) -> &Arc<AppMetrics> {
        &self.metrics
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    pub fn store(&self) -> &R {
        &self.store
    }

    pub fn access_buffer(&self) -> &AccessBuffer {
        &self.access_buffer
    }
}

/// Build the service router for the provided repository implementation.
pub fn app<R>(state: AppState<R>) -> Router
where
    R: LinkStore,
{
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz::<R>))
        .route("/metrics", get(metrics_exposition::<R>))
        .route("/stat", get(list_links::<R>))
        .route("/gen", post(create_link::<R>))
        .route(
            "/{hash}",
            get(redirect_to_link::<R>).delete(delete_link::<R>),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            track_request_metrics::<R>,
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn track_request_metrics<R>(
    State(state): State<AppState<R>>,
    request: Request,
    next: Next,
) -> Response
where
    R: LinkStore,
{
    state
        .metrics
        .http_requests_total
        .fetch_add(1, Ordering::Relaxed);
    next.run(request).await
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Serialize)]
struct ReadyResponse {
    status: &'static str,
    database: &'static str,
}

async fn healthz() -> (StatusCode, Json<HealthResponse>) {
    (StatusCode::OK, Json(HealthResponse { status: "ok" }))
}

async fn readyz<R>(State(state): State<AppState<R>>) -> (StatusCode, Json<ReadyResponse>)
where
    R: LinkStore,
{
    match state.store.ping().await {
        Ok(()) => (
            StatusCode::OK,
            Json(ReadyResponse {
                status: "ready",
                database: "connected",
            }),
        ),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ReadyResponse {
                status: "unready",
                database: "disconnected",
            }),
        ),
    }
}

async fn metrics_exposition<R>(State(state): State<AppState<R>>) -> Response
where
    R: LinkStore,
{
    (
        [(header::CONTENT_TYPE, PROMETHEUS_CONTENT_TYPE)],
        state.metrics.render_prometheus(),
    )
        .into_response()
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
                state
                    .metrics
                    .links_created_total
                    .fetch_add(1, Ordering::Relaxed);
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

    state.access_buffer.discard(&hash);
    state.access_cache.invalidate(&hash);
    let deleted = state.store.delete_link(&hash).await?;
    if deleted {
        state
            .metrics
            .links_deleted_total
            .fetch_add(1, Ordering::Relaxed);
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
    if let Some(cached) = state.access_cache.get(&hash, accessed_at) {
        state
            .metrics
            .cache_hits_total
            .fetch_add(1, Ordering::Relaxed);
        state.access_buffer.record(hash, accessed_at);
        state
            .metrics
            .redirects_total
            .fetch_add(1, Ordering::Relaxed);
        return Ok(Redirect::temporary(&cached.original_url));
    }

    state
        .metrics
        .cache_misses_total
        .fetch_add(1, Ordering::Relaxed);
    let Some(link) = state.store.get_active_link(&hash, accessed_at).await? else {
        return Err(AppError::NotFound);
    };

    state.access_buffer.record(link.hash.clone(), accessed_at);
    state.access_cache.remember(&link);
    state
        .metrics
        .redirects_total
        .fetch_add(1, Ordering::Relaxed);
    Ok(Redirect::temporary(&link.original_url))
}

async fn list_links<R>(
    State(state): State<AppState<R>>,
    headers: HeaderMap,
    Query(params): Query<StatQueryParams>,
) -> Result<Json<PaginatedLinkStatsResponse>, AppError>
where
    R: LinkStore,
{
    authorize(&headers, &state.config.app_key)?;

    state.access_buffer.flush(&state.store).await?;

    let limit = params.normalized_limit();
    let offset = params.offset;
    let (documents, total) = state
        .store
        .list_links(DateTime::now(), limit, offset)
        .await?;

    Ok(Json(PaginatedLinkStatsResponse {
        items: to_stats_responses(documents, &state.config.app_hostname),
        total,
        limit,
        offset,
    }))
}

fn to_stats_responses(documents: Vec<LinkDocument>, app_hostname: &str) -> Vec<LinkStatsResponse> {
    documents
        .into_iter()
        .map(|document| document.into_stats_response(app_hostname))
        .collect()
}
