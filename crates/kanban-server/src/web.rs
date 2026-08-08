//! `kanban serve` 的同源 Web host 装配。
//!
//! 这里只负责把已经校验并冻结的 [`kanban_web_artifact::VerifiedWebArtifact`]
//! 投影成 HTTP。浏览器和 Tauri 都从同一个 immutable snapshot 读取；请求处理期间
//! 不重新打开 dist，也不使用 `ServeDir`。

use std::{net::SocketAddr, sync::Arc};

use axum::{
    Router,
    body::Body,
    extract::{OriginalUri, State},
    http::{
        HeaderMap, HeaderValue, Method, Request, Response, StatusCode, Uri,
        header::{self, HeaderName},
    },
    middleware::{self, Next},
    response::Redirect,
    routing::get,
};
use kanban_protocol::{
    WEB_ARTIFACT_BASE_PATH, WEB_ARTIFACT_ENTRYPOINT, WEB_PROTOCOL_VERSION, WebRuntimeConfig,
    web_artifact_sha256_for_bytes,
};
use kanban_web_artifact::{VerifiedWebArtifact, VerifiedWebArtifactPayload};

const APP_PATH: &str = "/app";
const APP_BASE_PATH: &str = "/app/";
const DEFAULT_CSP: &str = concat!(
    "default-src 'self'; ",
    "script-src 'self'; ",
    "style-src 'self'; ",
    "img-src 'self' data:; ",
    "font-src 'self'; ",
    "connect-src 'self'; ",
    "object-src 'none'; ",
    "base-uri 'self'; ",
    "frame-ancestors 'none'"
);

/// 生产 Web host 所需的 immutable artifact 与运行时 metadata。
#[derive(Clone)]
pub struct WebHostConfig {
    artifact: Arc<VerifiedWebArtifact>,
    runtime_bytes: Arc<[u8]>,
    runtime_etag: Arc<str>,
}

impl WebHostConfig {
    /// 为当前 host 创建 `/app/runtime.json` 的 typed wire projection。
    pub fn new(
        artifact: Arc<VerifiedWebArtifact>,
        actor: impl Into<String>,
        default_board: impl Into<String>,
    ) -> Self {
        let runtime = WebRuntimeConfig {
            api_base_url: String::new(),
            web_base_path: WEB_ARTIFACT_BASE_PATH.to_owned(),
            actor: actor.into(),
            default_board: default_board.into(),
            server_version: env!("CARGO_PKG_VERSION").to_owned(),
            protocol_version: WEB_PROTOCOL_VERSION.to_owned(),
            web_build_id: artifact.manifest().build_id.clone(),
        };
        let runtime_bytes = Arc::<[u8]>::from(
            serde_json::to_vec(&runtime)
                .expect("WebRuntimeConfig only contains serializable strings"),
        );
        let runtime_etag =
            Arc::<str>::from(quoted_etag(&web_artifact_sha256_for_bytes(&runtime_bytes)));
        Self {
            artifact,
            runtime_bytes,
            runtime_etag,
        }
    }

    /// 返回本次 host 使用的 immutable artifact snapshot。
    pub fn artifact(&self) -> &Arc<VerifiedWebArtifact> {
        &self.artifact
    }
}

#[derive(Clone)]
struct WebHostState {
    config: WebHostConfig,
}

#[derive(Clone)]
pub(crate) struct HostOriginPolicy {
    port: u16,
}

impl HostOriginPolicy {
    pub(crate) fn for_listener(listener: SocketAddr) -> Self {
        Self {
            port: listener.port(),
        }
    }

    fn accepts_host(&self, raw: &str) -> bool {
        let Ok(authority) = raw.parse::<axum::http::uri::Authority>() else {
            return false;
        };
        authority.port_u16() == Some(self.port) && matches_loopback_host(authority.host())
    }

    fn accepts_origin(&self, raw: &str) -> bool {
        if matches!(
            raw,
            "tauri://localhost" | "http://tauri.localhost" | "https://tauri.localhost"
        ) {
            return true;
        }
        let Ok(uri) = raw.parse::<Uri>() else {
            return false;
        };
        let Some(scheme) = uri.scheme_str() else {
            return false;
        };
        let Some(authority) = uri.authority() else {
            return false;
        };
        if !uri.path().is_empty() && uri.path() != "/" || uri.query().is_some() {
            return false;
        }
        if !matches!(scheme, "http" | "https") {
            return false;
        }
        let port = authority.port_u16();
        let host = authority.host();
        if matches_loopback_host(host) && port == Some(self.port) {
            return scheme == "http";
        }
        scheme == "http" && matches_loopback_host(host) && matches!(port, Some(1420 | 1421))
    }
}

fn matches_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host == "127.0.0.1"
        || host == "::1"
        || host == "[::1]"
}

/// 构造带 Web artifact 的生产 router；`build_router` 仍保持 API-only seam。
pub(crate) fn build_production_router(
    state: crate::state::AppState,
    config: WebHostConfig,
    listener: SocketAddr,
) -> Router {
    let host_state = Arc::new(WebHostState { config });
    let web = web_router(host_state);
    crate::http::operations::router(state)
        .merge(web)
        .layer(production_cors_layer(&HostOriginPolicy::for_listener(
            listener,
        )))
        .layer(middleware::from_fn(validate_web_path))
        .layer(middleware::from_fn_with_state(
            HostOriginPolicy::for_listener(listener),
            enforce_host_origin,
        ))
        .layer(tower_http::trace::TraceLayer::new_for_http())
}

fn web_router(state: Arc<WebHostState>) -> Router {
    let app = Router::new()
        .route("/", get(index))
        .route("/runtime.json", get(runtime))
        .route("/manifest.json", get(manifest))
        .fallback(app_fallback);
    Router::new()
        .route("/", get(root_redirect))
        .route("/app", get(app_redirect))
        .nest(APP_BASE_PATH, app)
        .with_state(state)
}

async fn root_redirect() -> Redirect {
    Redirect::temporary(APP_BASE_PATH)
}

async fn app_redirect() -> Redirect {
    Redirect::temporary(APP_BASE_PATH)
}

async fn index(
    method: Method,
    State(state): State<Arc<WebHostState>>,
    headers: HeaderMap,
) -> Response<Body> {
    payload_response(
        &method,
        state
            .config
            .artifact
            .payload(WEB_ARTIFACT_ENTRYPOINT)
            .expect("verified artifact always contains entrypoint"),
        "text/html; charset=utf-8",
        "no-cache",
        &headers,
    )
}

async fn runtime(
    method: Method,
    State(state): State<Arc<WebHostState>>,
    headers: HeaderMap,
) -> Response<Body> {
    bytes_response(
        &method,
        &state.config.runtime_bytes,
        &state.config.runtime_etag,
        "application/json; charset=utf-8",
        "no-store",
        &headers,
    )
}

async fn manifest(
    method: Method,
    State(state): State<Arc<WebHostState>>,
    headers: HeaderMap,
) -> Response<Body> {
    bytes_response(
        &method,
        state.config.artifact.manifest_bytes(),
        &quoted_etag(state.config.artifact.manifest_sha256()),
        "application/json; charset=utf-8",
        "no-cache",
        &headers,
    )
}

async fn app_fallback(
    method: Method,
    OriginalUri(uri): OriginalUri,
    State(state): State<Arc<WebHostState>>,
    headers: HeaderMap,
) -> Response<Body> {
    let Some(relative) = uri.path().strip_prefix(APP_BASE_PATH) else {
        return plain_status(StatusCode::NOT_FOUND);
    };
    if relative.is_empty() {
        if method != Method::GET && method != Method::HEAD {
            return plain_status(StatusCode::METHOD_NOT_ALLOWED);
        }
        return index(method, State(state), headers).await;
    }
    if let Some(payload) = state.config.artifact.payload(relative) {
        if method != Method::GET && method != Method::HEAD {
            return plain_status(StatusCode::METHOD_NOT_ALLOWED);
        }
        return payload_response(
            &method,
            payload,
            content_type(relative),
            "public, max-age=31536000, immutable",
            &headers,
        );
    }
    if relative == "assets"
        || relative.starts_with("assets/")
        || relative.split('/').any(|segment| segment.contains('.'))
    {
        return plain_status(StatusCode::NOT_FOUND);
    }
    if method != Method::GET && method != Method::HEAD {
        return plain_status(StatusCode::METHOD_NOT_ALLOWED);
    }
    payload_response(
        &method,
        state
            .config
            .artifact
            .payload(WEB_ARTIFACT_ENTRYPOINT)
            .expect("verified artifact always contains entrypoint"),
        "text/html; charset=utf-8",
        "no-cache",
        &headers,
    )
}

fn payload_response(
    method: &Method,
    payload: &VerifiedWebArtifactPayload,
    content_type: &str,
    cache_control: &str,
    headers: &HeaderMap,
) -> Response<Body> {
    let etag = quoted_etag(&payload.descriptor().sha256);
    bytes_response(
        method,
        &payload.bytes_arc(),
        &etag,
        content_type,
        cache_control,
        headers,
    )
}

fn bytes_response(
    method: &Method,
    bytes: &[u8],
    etag: &str,
    content_type: &str,
    cache_control: &str,
    headers: &HeaderMap,
) -> Response<Body> {
    if if_none_match(headers, etag) {
        let mut response = plain_status(StatusCode::NOT_MODIFIED);
        set_web_headers(response.headers_mut());
        response.headers_mut().insert(
            header::ETAG,
            HeaderValue::from_str(etag).expect("SHA-256 ETag is a valid header"),
        );
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_str(cache_control).expect("cache policy is a valid header"),
        );
        response
    } else {
        let mut response = Response::new(if method == Method::HEAD {
            Body::empty()
        } else {
            Body::from(bytes.to_owned())
        });
        *response.status_mut() = StatusCode::OK;
        set_web_headers(response.headers_mut());
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_str(content_type).expect("known MIME is a valid header"),
        );
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_str(cache_control).expect("cache policy is a valid header"),
        );
        response.headers_mut().insert(
            header::CONTENT_LENGTH,
            HeaderValue::from_str(&bytes.len().to_string()).expect("length is a valid header"),
        );
        response.headers_mut().insert(
            header::ETAG,
            HeaderValue::from_str(etag).expect("SHA-256 ETag is a valid header"),
        );
        response
    }
}

fn if_none_match(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(',')
                .any(|candidate| candidate.trim() == etag || candidate.trim() == "*")
        })
        .unwrap_or(false)
}

fn quoted_etag(digest: &str) -> String {
    format!("\"{digest}\"")
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "wasm" => "application/wasm",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn set_web_headers(headers: &mut HeaderMap) {
    headers.insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static(DEFAULT_CSP),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );
}

fn plain_status(status: StatusCode) -> Response<Body> {
    Response::builder()
        .status(status)
        .body(Body::empty())
        .expect("empty status response should build")
}

async fn validate_web_path(request: Request<Body>, next: Next) -> Response<Body> {
    let path = request.uri().path();
    if (path == "/" || path == APP_PATH || path.starts_with(APP_BASE_PATH))
        && (path
            .as_bytes()
            .iter()
            .any(|byte| matches!(byte, b'%' | b'\\' | 0))
            || path.split('/').any(|segment| matches!(segment, "." | "..")))
    {
        return plain_status(StatusCode::BAD_REQUEST);
    }
    next.run(request).await
}

async fn enforce_host_origin(
    State(policy): State<HostOriginPolicy>,
    request: Request<Body>,
    next: Next,
) -> Response<Body> {
    let host_ok = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|host| policy.accepts_host(host));
    let origin_ok = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .is_none_or(|origin| policy.accepts_origin(origin));
    if !host_ok || !origin_ok {
        return plain_status(StatusCode::BAD_REQUEST);
    }
    next.run(request).await
}

fn production_cors_layer(policy: &HostOriginPolicy) -> tower_http::cors::CorsLayer {
    use tower_http::cors::{AllowOrigin, CorsLayer};
    let mut origins = vec![
        HeaderValue::from_static("http://127.0.0.1:1420"),
        HeaderValue::from_static("http://localhost:1420"),
        HeaderValue::from_static("http://127.0.0.1:1421"),
        HeaderValue::from_static("http://localhost:1421"),
        HeaderValue::from_static("http://tauri.localhost"),
        HeaderValue::from_static("https://tauri.localhost"),
        HeaderValue::from_static("tauri://localhost"),
    ];
    for host in ["localhost", "127.0.0.1", "[::1]"] {
        let value = format!("http://{host}:{}", policy.port);
        if let Ok(value) = HeaderValue::from_str(&value) {
            origins.push(value);
        }
    }
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::ACCEPT,
            HeaderName::from_static("last-event-id"),
            HeaderName::from_static("x-kb-actor"),
        ])
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        net::{IpAddr, Ipv4Addr, SocketAddr},
        path::Path,
    };

    use axum::{
        body::Body,
        http::{Request, StatusCode, header},
    };
    use http_body_util::BodyExt;
    use kanban_protocol::{
        WEB_ARTIFACT_BASE_PATH, WEB_ARTIFACT_ENTRYPOINT, WEB_ARTIFACT_FORMAT_VERSION,
        WEB_PROTOCOL_VERSION, WebArtifactFile, WebArtifactManifest, WebRuntimeConfig,
        web_artifact_build_id_for, web_artifact_file_from_bytes,
    };
    use kanban_web_artifact::verify_directory;
    use serde_json::Value;
    use tempfile::TempDir;
    use tower::ServiceExt;

    use super::{APP_BASE_PATH, HostOriginPolicy, WebHostConfig, build_production_router};
    use crate::AppState;

    const PORT: u16 = 9876;

    fn write_artifact(root: &Path) -> Vec<u8> {
        let payloads = [
            ("index.html", b"<main id=\"root\"></main>".as_slice()),
            ("assets/app.js", b"console.log('ok');".as_slice()),
        ];
        let mut files = payloads
            .iter()
            .map(|(path, bytes)| {
                let target = root.join(path);
                fs::create_dir_all(target.parent().expect("payload parent")).expect("mkdir");
                fs::write(&target, bytes).expect("write payload");
                web_artifact_file_from_bytes(path, bytes).expect("descriptor")
            })
            .collect::<Vec<WebArtifactFile>>();
        files.sort_by(|left, right| left.path.cmp(&right.path));
        let build_id = web_artifact_build_id_for(
            WEB_ARTIFACT_FORMAT_VERSION,
            WEB_ARTIFACT_BASE_PATH,
            WEB_ARTIFACT_ENTRYPOINT,
            env!("CARGO_PKG_VERSION"),
            WEB_PROTOCOL_VERSION,
            &files,
        )
        .expect("build id");
        let manifest = WebArtifactManifest {
            format_version: WEB_ARTIFACT_FORMAT_VERSION,
            base_path: WEB_ARTIFACT_BASE_PATH.to_owned(),
            entrypoint: WEB_ARTIFACT_ENTRYPOINT.to_owned(),
            server_version: env!("CARGO_PKG_VERSION").to_owned(),
            protocol_version: WEB_PROTOCOL_VERSION.to_owned(),
            build_id,
            files,
        };
        let bytes = serde_json::to_vec(&manifest).expect("manifest json");
        fs::write(root.join("manifest.json"), &bytes).expect("write manifest");
        bytes
    }

    async fn app() -> (TempDir, TempDir, axum::Router) {
        let artifact_root = tempfile::tempdir().expect("artifact tempdir");
        let raw_manifest = write_artifact(artifact_root.path());
        let artifact = verify_directory(artifact_root.path(), env!("CARGO_PKG_VERSION"))
            .expect("verify artifact");
        assert_eq!(artifact.manifest_bytes(), raw_manifest.as_slice());
        let db_root = tempfile::tempdir().expect("database tempdir");
        let state = AppState::open(db_root.path().join("kanban.db"), "test-actor")
            .await
            .expect("open state");
        let config = WebHostConfig::new(std::sync::Arc::new(artifact), "test-actor", "default");
        let router = build_production_router(
            state,
            config,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), PORT),
        );
        (artifact_root, db_root, router)
    }

    async fn response(
        router: &mut axum::Router,
        method: &str,
        path: &str,
        extra_headers: &[(&str, &str)],
    ) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
        let mut builder = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, format!("localhost:{PORT}"));
        for (name, value) in extra_headers {
            builder = builder.header(*name, *value);
        }
        let response = router
            .oneshot(builder.body(Body::empty()).expect("request"))
            .await
            .expect("response");
        let status = response.status();
        let headers = response.headers().clone();
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes()
            .to_vec();
        (status, headers, body)
    }

    #[tokio::test]
    async fn serves_redirects_runtime_manifest_assets_and_spa_routes() {
        let (_artifact_root, _db_root, mut router) = app().await;
        let (status, headers, body) = response(&mut router, "GET", "/", &[]).await;
        assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(headers[header::LOCATION], APP_BASE_PATH);
        assert!(body.is_empty());

        let (status, headers, body) = response(&mut router, "GET", "/app", &[]).await;
        assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(headers[header::LOCATION], APP_BASE_PATH);
        assert!(body.is_empty());

        let (status, headers, body) = response(&mut router, "GET", "/app/", &[]).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, b"<main id=\"root\"></main>");
        assert_eq!(headers[header::CONTENT_TYPE], "text/html; charset=utf-8");
        assert_eq!(headers[header::CACHE_CONTROL], "no-cache");
        assert!(
            headers["content-security-policy"]
                .to_str()
                .unwrap()
                .contains("script-src 'self'")
        );
        assert!(
            !headers["content-security-policy"]
                .to_str()
                .unwrap()
                .contains("unsafe-inline")
        );

        let (status, headers, body) = response(&mut router, "GET", "/app/runtime.json", &[]).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CACHE_CONTROL], "no-store");
        let runtime: WebRuntimeConfig = serde_json::from_slice(&body).expect("runtime config");
        assert_eq!(runtime.api_base_url, "");
        assert_eq!(runtime.web_base_path, APP_BASE_PATH);
        assert_eq!(runtime.actor, "test-actor");
        assert_eq!(runtime.default_board, "default");
        assert_eq!(runtime.protocol_version, "v1");
        assert!(runtime.web_build_id.starts_with("sha256:"));

        let (status, headers, body) = response(&mut router, "GET", "/app/manifest.json", &[]).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CACHE_CONTROL], "no-cache");
        let manifest: Value = serde_json::from_slice(&body).expect("manifest json");
        assert_eq!(manifest["entrypoint"], WEB_ARTIFACT_ENTRYPOINT);

        let (status, headers, body) = response(&mut router, "GET", "/app/assets/app.js", &[]).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, b"console.log('ok');");
        assert_eq!(
            headers[header::CONTENT_TYPE],
            "text/javascript; charset=utf-8"
        );
        assert_eq!(
            headers[header::CACHE_CONTROL],
            "public, max-age=31536000, immutable"
        );

        let (status, _headers, body) =
            response(&mut router, "GET", "/app/board/default", &[]).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, b"<main id=\"root\"></main>");
        let (status, _headers, body) = response(&mut router, "GET", "/app/missing.js", &[]).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body.is_empty());
        let (status, _headers, body) = response(&mut router, "GET", "/app/assets", &[]).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body.is_empty());
        let (status, _headers, body) =
            response(&mut router, "GET", "/app/assets/missing", &[]).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body.is_empty());
        let (status, _headers, body) =
            response(&mut router, "POST", "/app/board/default", &[]).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert!(body.is_empty());
        let (status, _headers, body) = response(&mut router, "POST", "/app/", &[]).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert!(body.is_empty());
        let (status, _headers, body) = response(&mut router, "GET", "/api/unknown", &[]).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_ne!(body, b"<main id=\"root\"></main>");
    }

    #[tokio::test]
    async fn preserves_head_length_and_returns_conditional_304() {
        let (_artifact_root, _db_root, mut router) = app().await;
        let (status, get_headers, get_body) = response(&mut router, "GET", "/app/", &[]).await;
        assert_eq!(status, StatusCode::OK);
        let (status, head_headers, head_body) = response(&mut router, "HEAD", "/app/", &[]).await;
        assert_eq!(status, StatusCode::OK);
        assert!(head_body.is_empty());
        assert_eq!(
            head_headers[header::CONTENT_LENGTH],
            get_body.len().to_string()
        );
        assert_eq!(head_headers[header::ETAG], get_headers[header::ETAG]);

        let etag = get_headers[header::ETAG].to_str().expect("etag");
        let (status, headers, body) =
            response(&mut router, "GET", "/app/", &[("if-none-match", etag)]).await;
        assert_eq!(status, StatusCode::NOT_MODIFIED);
        assert!(body.is_empty());
        assert_eq!(headers[header::ETAG], etag);
    }

    #[tokio::test]
    async fn rejects_unsafe_paths_and_enforces_host_origin_allowlist() {
        let (_artifact_root, _db_root, mut router) = app().await;
        for path in [
            "/app/%2e%2e/index.html",
            "/app/a\\b",
            "/app/./x",
            "/app/../x",
        ] {
            let (status, _headers, _body) = response(&mut router, "GET", path, &[]).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        }
        let request = Request::get("/app/")
            .header(header::HOST, format!("localhost:{}", PORT + 1))
            .body(Body::empty())
            .expect("request");
        let rejected = router.clone().oneshot(request).await.expect("response");
        assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);

        let (status, _, _) = response(
            &mut router,
            "GET",
            "/app/",
            &[("origin", "http://localhost:1421")],
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _, _) = response(
            &mut router,
            "GET",
            "/app/",
            &[("origin", "http://tauri.localhost")],
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _, _) = response(
            &mut router,
            "GET",
            "/app/",
            &[("origin", "http://example.invalid")],
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        let preflight = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri("/api/v1/stream/events")
                    .header(header::HOST, format!("localhost:{PORT}"))
                    .header(header::ORIGIN, format!("http://localhost:{PORT}"))
                    .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                    .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "last-event-id")
                    .body(Body::empty())
                    .expect("preflight request"),
            )
            .await
            .expect("preflight response");
        assert_eq!(preflight.status(), StatusCode::OK);
        assert!(
            preflight.headers()[header::ACCESS_CONTROL_ALLOW_HEADERS]
                .to_str()
                .expect("allow headers")
                .split(',')
                .any(|value| value.trim().eq_ignore_ascii_case("last-event-id"))
        );

        let policy = HostOriginPolicy::for_listener(SocketAddr::from(([127, 0, 0, 1], 9988)));
        assert!(policy.accepts_host("[::1]:9988"));
        assert!(policy.accepts_origin("http://[::1]:9988"));
        assert!(!policy.accepts_origin("https://localhost:1420"));
        assert!(!policy.accepts_host("127.0.0.1:9987"));
    }
}
