use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request, header},
    middleware::Next,
    response::Response,
};
use kanban_core::Locale;

use crate::state::AppState;

tokio::task_local! {
    static REQUEST_LOCALE: Locale;
}

pub(crate) async fn request_locale(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request<Body>,
    next: Next,
) -> Response {
    let locale = Locale::from_accept_language(
        headers
            .get(header::ACCEPT_LANGUAGE)
            .and_then(|value| value.to_str().ok()),
        state.locale(),
    );
    REQUEST_LOCALE
        .scope(locale, async move { next.run(request).await })
        .await
}

pub(crate) fn current_request_locale() -> Locale {
    REQUEST_LOCALE
        .try_with(|locale| *locale)
        .unwrap_or(Locale::En)
}
