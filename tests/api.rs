use std::sync::Arc;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use rlnk::{
    config::AppConfig,
    http::{AppState, app},
    model::{CreateLinkResponse, LinkStatsResponse},
    store::MemoryLinkStore,
};
use serde::de::DeserializeOwned;
use tower::ServiceExt;

fn test_app() -> Router {
    let config = Arc::new(
        AppConfig::from_pairs([
            ("MONGO_URI", "mongodb://localhost:27017"),
            ("APP_KEY", "test-key"),
            ("APP_HOSTNAME", "https://rlnk.test"),
        ])
        .expect("test config should load"),
    );

    app(AppState::new(config, MemoryLinkStore::new()))
}

fn authed_request(method: &str, uri: &str, body: Body) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, "test-key")
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

    let create_response = app
        .clone()
        .oneshot(authed_request(
            "POST",
            "/gen",
            Body::from(r#"{"url":"https://example.com/path","ttl":"10m"}"#),
        ))
        .await
        .expect("create request should complete");
    assert_eq!(create_response.status(), StatusCode::OK);
    let created_link: CreateLinkResponse = read_json(create_response).await;

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
    let stats: Vec<LinkStatsResponse> = read_json(stats_response).await;

    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].hash, created_link.hash);
    assert_eq!(stats[0].access_count, 1);
    assert!(stats[0].last_accessed_at.is_some());
}

#[tokio::test]
async fn delete_hash_should_remove_link_and_make_follow_up_lookup_fail() {
    let app = test_app();

    let create_response = app
        .clone()
        .oneshot(authed_request(
            "POST",
            "/gen",
            Body::from(r#"{"url":"https://example.com/delete-me"}"#),
        ))
        .await
        .expect("create request should complete");
    let created_link: CreateLinkResponse = read_json(create_response).await;

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
