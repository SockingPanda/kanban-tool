use std::{
    future::Future,
    io::{self, Write},
    sync::{Arc, Mutex},
};

use crate::common::*;
use tracing_subscriber::{EnvFilter, fmt::MakeWriter};

#[derive(Clone, Default)]
struct CapturedLogs(Arc<Mutex<Vec<u8>>>);

impl CapturedLogs {
    fn contents(&self) -> String {
        let bytes = self.0.lock().expect("captured logs lock poisoned").clone();
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

struct CapturedLogWriter(Arc<Mutex<Vec<u8>>>);

impl Write for CapturedLogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .expect("captured logs lock poisoned")
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'writer> MakeWriter<'writer> for CapturedLogs {
    type Writer = CapturedLogWriter;

    fn make_writer(&'writer self) -> Self::Writer {
        CapturedLogWriter(self.0.clone())
    }
}

async fn with_default_trace_capture<F, T>(logs: CapturedLogs, future: F) -> T
where
    F: Future<Output = T>,
{
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(kanban_server::DEFAULT_TRACING_FILTER))
        .with_writer(logs)
        .with_ansi(false)
        .without_time()
        .finish();
    let dispatch = tracing::Dispatch::new(subscriber);
    let guard = tracing::dispatcher::set_default(&dispatch);
    let output = future.await;
    drop(guard);
    output
}

#[tokio::test(flavor = "current_thread")]
async fn serve_router_traces_database_guard_short_circuit_at_default_filter() -> anyhow::Result<()>
{
    let test = TestApp::new()?;
    std::fs::remove_file(test.db_path()).context("remove db")?;
    let app = test.serve_router();
    let logs = CapturedLogs::default();

    let response = with_default_trace_capture(logs.clone(), async move {
        app.oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .context("request")?,
        )
        .await
        .context("response")
    })
    .await?;
    let (status, _json) = response_json(response).await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    let output = logs.contents();
    assert!(
        output.contains("started processing request"),
        "expected request trace in logs, got:\n{output}"
    );
    assert!(
        output.contains("finished processing request"),
        "expected response trace in logs, got:\n{output}"
    );
    assert!(
        output.contains("status=400"),
        "expected short-circuit status in logs, got:\n{output}"
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn desktop_router_traces_cors_preflight_short_circuit_at_default_filter() -> anyhow::Result<()>
{
    let test = TestApp::new()?;
    let app = test.desktop_router();
    let logs = CapturedLogs::default();

    let (status, headers) = with_default_trace_capture(logs.clone(), async move {
        options_raw(app, "/api/v1/boards/default/tasks", "http://127.0.0.1:1420").await
    })
    .await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
        Some(&HeaderValue::from_static("http://127.0.0.1:1420"))
    );
    let output = logs.contents();
    assert!(
        output.contains("started processing request"),
        "expected request trace in logs, got:\n{output}"
    );
    assert!(
        output.contains("finished processing request"),
        "expected response trace in logs, got:\n{output}"
    );
    assert!(
        output.contains("status=200"),
        "expected CORS preflight status in logs, got:\n{output}"
    );
    Ok(())
}
