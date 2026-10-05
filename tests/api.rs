use std::sync::Arc;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use rlnk::{
    config::AppConfig,
    http::{AppState, app},
    model::{CreateLinkResponse, PaginatedLinkStatsResponse},
    store::MemoryLinkStore,
};

use serde::de::DeserializeOwned;
use tower::ServiceExt;

const TEST_APP_KEY: &str = "test-key";
const TEST_AUTHORIZATION: &str = "Bearer test-key";

fn test_app() -> Router {
    test_app_with_cache_size("1024")
}

fn test_app_with_cache_size(access_cache_size: &str) -> Router {
    let config = Arc::new(
        AppConfig::from_pairs([
            ("MONGO_URI", "mongodb://localhost:27017"),
            ("APP_KEY", TEST_APP_KEY),
            ("APP_HOSTNAME", "https://rlnk.test"),
            ("ACCESS_CACHE_SIZE", access_cache_size),
        ])
        .expect("test config should load"),
    );

    app(AppState::new(config, MemoryLinkStore::new()))
}

async fn create_link(app: &Router, body: &'static str) -> CreateLinkResponse {
    let create_response = app
        .clone()
        .oneshot(authed_request("POST", "/gen", Body::from(body)))
        .await
        .expect("create request should complete");
    assert_eq!(create_response.status(), StatusCode::OK);

    read_json(create_response).await
}

fn authed_request(method: &str, uri: &str, body: Body) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, TEST_AUTHORIZATION)
        .header(header::CONTENT_TYPE, "application/json")
        .body(body)
        .expect("request should build")
}

async fn read_json<T>(response: axum::response::Response) -> T
where
    T: DeserializeOwned,
{
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should read");
    serde_json::from_slice(&bytes).expect("body should deserialize")
}

#[tokio::test]
async fn post_gen_should_reject_raw_authorization_key() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/gen")
                .header(header::AUTHORIZATION, TEST_APP_KEY)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"url":"https://example.com"}"#))
                .expect("request should build"),
        )
        .await
        .expect("request should complete");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn post_gen_should_reject_missing_authorization_header() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/gen")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"url":"https://example.com"}"#))
                .expect("request should build"),
        )
        .await
        .expect("request should complete");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn get_hash_should_redirect_and_update_stats_after_link_is_created() {
    let app = test_app();

    let created_link = create_link(&app, r#"{"url":"https://example.com/path","ttl":"10m"}"#).await;

    let redirect_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/{}", created_link.hash))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("redirect request should complete");
    assert_eq!(redirect_response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        redirect_response.headers().get(header::LOCATION),
        Some(&header::HeaderValue::from_static(
            "https://example.com/path"
        ))
    );

    let stats_response = app
        .oneshot(authed_request("GET", "/stat", Body::empty()))
        .await
        .expect("stats request should complete");
    assert_eq!(stats_response.status(), StatusCode::OK);
    let stats: PaginatedLinkStatsResponse = read_json(stats_response).await;

    assert_eq!(stats.total, 1);
    assert_eq!(stats.items.len(), 1);
    assert_eq!(stats.items[0].hash, created_link.hash);
    assert_eq!(stats.items[0].access_count, 1);
    assert!(stats.items[0].last_accessed_at.is_some());
}

#[tokio::test]
async fn get_hash_should_update_stats_when_recent_access_cache_hits() {
    let app = test_app_with_cache_size("1");
    let created_link = create_link(&app, r#"{"url":"https://example.com/cached"}"#).await;

    for _ in 0..2 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/{}", created_link.hash))
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("redirect request should complete");
        assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    }

    let stats_response = app
        .oneshot(authed_request("GET", "/stat", Body::empty()))
        .await
        .expect("stats request should complete");
    let stats: PaginatedLinkStatsResponse = read_json(stats_response).await;

    assert_eq!(stats.total, 1);
    assert_eq!(stats.items[0].access_count, 2);
}

#[tokio::test]
async fn delete_hash_should_remove_link_and_make_follow_up_lookup_fail() {
    let app = test_app();

    let created_link = create_link(&app, r#"{"url":"https://example.com/delete-me"}"#).await;

    let delete_response = app
        .clone()
        .oneshot(authed_request(
            "DELETE",
            &format!("/{}", created_link.hash),
            Body::empty(),
        ))
        .await
        .expect("delete request should complete");
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    let lookup_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/{}", created_link.hash))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("lookup request should complete");

    assert_eq!(lookup_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_hash_should_invalidate_recent_access_cache() {
    let app = test_app_with_cache_size("1");
    let created_link = create_link(&app, r#"{"url":"https://example.com/delete-cached"}"#).await;

    let warm_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/{}", created_link.hash))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("warm request should complete");
    assert_eq!(warm_response.status(), StatusCode::TEMPORARY_REDIRECT);

    let delete_response = app
        .clone()
        .oneshot(authed_request(
            "DELETE",
            &format!("/{}", created_link.hash),
            Body::empty(),
        ))
        .await
        .expect("delete request should complete");
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    let lookup_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/{}", created_link.hash))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("lookup request should complete");

    assert_eq!(lookup_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_stat_should_reject_missing_authorization_header() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/stat")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should complete");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn healthz_should_return_ok_without_auth() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/healthz")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should complete");

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = read_json(response).await;
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn readyz_should_return_ready_without_auth() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/readyz")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should complete");

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = read_json(response).await;
    assert_eq!(body["status"], "ready");
    assert_eq!(body["database"], "connected");
}

#[tokio::test]
async fn metrics_should_return_prometheus_text_without_auth() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/metrics")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should complete");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("text/plain; version=0.0.4; charset=utf-8")
    );

    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should read");
    let text = String::from_utf8(bytes.to_vec()).expect("body should be utf8");
    assert!(text.contains("rlnk_http_requests_total"));
    assert!(text.contains("rlnk_redirects_total"));
}

#[tokio::test]
async fn get_stat_should_support_limit_and_offset_pagination() {
    let app = test_app();

    let link1 = create_link(&app, r#"{"url":"https://example.com/one"}"#).await;
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    let link2 = create_link(&app, r#"{"url":"https://example.com/two"}"#).await;
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    let link3 = create_link(&app, r#"{"url":"https://example.com/three"}"#).await;

    // First page with limit 2
    let page1_response = app
        .clone()
        .oneshot(authed_request(
            "GET",
            "/stat?limit=2&offset=0",
            Body::empty(),
        ))
        .await
        .expect("page1 request should complete");
    assert_eq!(page1_response.status(), StatusCode::OK);
    let page1: PaginatedLinkStatsResponse = read_json(page1_response).await;

    assert_eq!(page1.total, 3);
    assert_eq!(page1.limit, 2);
    assert_eq!(page1.offset, 0);
    assert_eq!(page1.items.len(), 2);
    assert_eq!(page1.items[0].hash, link3.hash);
    assert_eq!(page1.items[1].hash, link2.hash);

    // Second page with offset 2
    let page2_response = app
        .oneshot(authed_request(
            "GET",
            "/stat?limit=2&offset=2",
            Body::empty(),
        ))
        .await
        .expect("page2 request should complete");
    assert_eq!(page2_response.status(), StatusCode::OK);
    let page2: PaginatedLinkStatsResponse = read_json(page2_response).await;

    assert_eq!(page2.total, 3);
    assert_eq!(page2.limit, 2);
    assert_eq!(page2.offset, 2);
    assert_eq!(page2.items.len(), 1);
    assert_eq!(page2.items[0].hash, link1.hash);
}

#[tokio::test]
async fn metrics_should_track_requests_and_redirects() {
    let app = test_app();

    let created_link = create_link(&app, r#"{"url":"https://example.com/tracked"}"#).await;

    let redirect_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/{}", created_link.hash))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("redirect request should complete");
    assert_eq!(redirect_response.status(), StatusCode::TEMPORARY_REDIRECT);

    let metrics_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/metrics")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("metrics request should complete");
    assert_eq!(metrics_response.status(), StatusCode::OK);

    let bytes = to_bytes(metrics_response.into_body(), usize::MAX)
        .await
        .expect("body should read");
    let text = String::from_utf8(bytes.to_vec()).expect("body should be utf8");

    assert!(text.contains("rlnk_links_created_total 1\n"));
    assert!(text.contains("rlnk_redirects_total 1\n"));
}
