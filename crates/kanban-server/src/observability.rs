use tower_http::trace::{
    DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, HttpMakeClassifier, TraceLayer,
};
use tracing::Level;
use tracing_subscriber::EnvFilter;

pub const DEFAULT_TRACING_FILTER: &str = "kanban_server=info,tower_http=info,kanban_desktop=info";

pub type HttpTraceLayer =
    TraceLayer<HttpMakeClassifier, DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse>;

pub fn http_trace_layer() -> HttpTraceLayer {
    TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
        .on_request(DefaultOnRequest::new().level(Level::INFO))
        .on_response(DefaultOnResponse::new().level(Level::INFO))
}

pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(DEFAULT_TRACING_FILTER));
    init_tracing_with_filter(filter);
}

pub fn init_tracing_with_filter(filter: EnvFilter) {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .try_init();
}
