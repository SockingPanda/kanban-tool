use crate::*;

pub(crate) async fn health(
    State(state): State<AppState>,
) -> Result<Json<Envelope<serde_json::Value>>, ApiError> {
    let _conn = kanban_sqlite::connect_file(state.db_path())?;
    Ok(Json(Envelope {
        data: json!({
            "ok": true,
            "db": "ok",
            "version": env!("CARGO_PKG_VERSION"),
        }),
        meta: None,
    }))
}
