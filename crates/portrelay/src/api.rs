use crate::agent::{Action, Agent};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Request, State},
    http::{HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use std::sync::Arc;
use subtle::ConstantTimeEq;

pub fn router(agent: Arc<Agent>) -> Router {
    let api = Router::new()
        .route("/api/state", get(state))
        .route("/api/action", post(action))
        .route_layer(middleware::from_fn_with_state(agent.clone(), authenticate));
    Router::new()
        .route(
            "/",
            get(|| async { Html(include_str!("../../../app-ui/index.html")) }),
        )
        .route(
            "/app.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../../../app-ui/app.js"),
                )
            }),
        )
        .route(
            "/input-events.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../../../app-ui/input-events.js"),
                )
            }),
        )
        .route(
            "/input.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../../../app-ui/input.js"),
                )
            }),
        )
        .route(
            "/style.css",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
                    include_str!("../../../app-ui/style.css"),
                )
            }),
        )
        .merge(api)
        .layer(DefaultBodyLimit::max(65536))
        .layer(middleware::from_fn(headers))
        .with_state(agent)
}
async fn authenticate(State(agent): State<Arc<Agent>>, request: Request, next: Next) -> Response {
    let provided = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .unwrap_or("");
    if !bool::from(provided.as_bytes().ct_eq(agent.token.as_bytes())) {
        return (StatusCode::UNAUTHORIZED,Json(serde_json::json!({"error":"Open PortRelay from its desktop shortcut or run portrelay open."}))).into_response();
    }
    if let Some(origin) = request.headers().get(header::ORIGIN) {
        let host = request
            .headers()
            .get(header::HOST)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");
        if origin.to_str().ok() != Some(format!("http://{host}").as_str())
            || !host.starts_with("127.0.0.1:")
        {
            return StatusCode::FORBIDDEN.into_response();
        }
    }
    next.run(request).await
}
async fn headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    for (k, v) in [
        ("cache-control", "no-store"),
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        ("referrer-policy", "no-referrer"),
        (
            "content-security-policy",
            "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; frame-ancestors 'none'; base-uri 'none'; form-action 'none'",
        ),
    ] {
        response
            .headers_mut()
            .insert(k, HeaderValue::from_static(v));
    }
    response
}
async fn state(State(agent): State<Arc<Agent>>) -> Json<serde_json::Value> {
    Json(agent.snapshot().await)
}
async fn action(State(agent): State<Arc<Agent>>, Json(action): Json<Action>) -> Response {
    match agent.action(action).await {
        Ok(value) => Json(value).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":e.to_string()})),
        )
            .into_response(),
    }
}
