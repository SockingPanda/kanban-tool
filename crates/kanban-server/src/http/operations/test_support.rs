pub(crate) use crate::{router::build_router, state::AppState};
pub(crate) use axum::{
    body::Body,
    http::{Request, StatusCode},
};
pub(crate) use http_body_util::BodyExt;
pub(crate) use kanban_protocol::*;
pub(crate) use tower::ServiceExt;

pub(crate) fn json_request(uri: &str, value: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&value).unwrap()))
        .unwrap()
}

pub(crate) fn patch_json_request(uri: &str, value: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("PATCH")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&value).unwrap()))
        .unwrap()
}
