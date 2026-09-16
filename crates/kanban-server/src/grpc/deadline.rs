//! Axum 内嵌 RPC 不经过 tonic transport Server，显式处理 grpc-timeout 到整个响应流。
use std::{
    convert::Infallible,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use axum::http::{HeaderMap, Request, Response};
use http_body::Frame;
use http_body_util::{BodyExt, StreamBody};
use tokio::time::Instant;
use tonic::{Status, body::Body};
use tower::Service;

#[derive(Clone)]
pub struct Deadline<S> {
    inner: S,
}

impl<S> Deadline<S> {
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S, B> Service<Request<B>> for Deadline<S>
where
    S: Service<Request<B>, Response = Response<Body>, Error = Infallible>,
    S::Future: Send + 'static,
{
    type Response = Response<Body>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<B>) -> Self::Future {
        let deadline = match parse(request.headers()) {
            Ok(deadline) => deadline,
            Err(error) => return Box::pin(async { Ok(error.into_http()) }),
        };
        if deadline.is_some_and(|deadline| deadline <= Instant::now()) {
            return Box::pin(async { Ok(expired().into_http()) });
        }
        let response = self.inner.call(request);
        Box::pin(async move {
            let Some(deadline) = deadline else {
                return response.await;
            };
            let response = match tokio::time::timeout_at(deadline, response).await {
                Ok(response) => response?,
                Err(_) => return Ok(expired().into_http()),
            };
            // Trailers-only 响应已结束，不再把成功/业务错误替换为超时。
            if response.headers().contains_key("grpc-status") {
                return Ok(response);
            }
            Ok(response.map(|mut body| Body::new(StreamBody::new(async_stream::stream! {
                loop {
                    match tokio::time::timeout_at(deadline, body.frame()).await {
                        Ok(Some(frame)) => {
                            let terminal = frame.as_ref().is_ok_and(|frame| frame.is_trailers());
                            yield frame;
                            if terminal { break; }
                        }
                        Ok(None) => break,
                        Err(_) => {
                            // 交给外层 tonic-web 编码成正常 terminal frame，同时释放原流和订阅许可。
                            drop(body);
                            let mut headers = HeaderMap::new();
                            expired().add_header(&mut headers).expect("固定超时状态可编码");
                            yield Ok::<_, Status>(Frame::trailers(headers));
                            break;
                        }
                    }
                }
            }))))
        })
    }
}

fn expired() -> Status {
    Status::deadline_exceeded("RPC deadline 已到期")
}

fn parse(headers: &HeaderMap) -> Result<Option<Instant>, Status> {
    let mut values = headers.get_all("grpc-timeout").iter();
    let Some(raw) = values.next() else {
        return Ok(None);
    };
    let invalid = || Status::invalid_argument("grpc-timeout 必须是至多八位数字和 H/M/S/m/u/n 单位");
    if values.next().is_some() {
        return Err(invalid());
    }
    let raw = raw.to_str().map_err(|_| invalid())?;
    if !(2..=9).contains(&raw.len()) {
        return Err(invalid());
    }
    let (number, unit) = raw.split_at(raw.len() - 1);
    if !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid());
    }
    let number = number.parse::<u64>().map_err(|_| invalid())?;
    let duration = match unit {
        "H" => Duration::from_secs(number * 3600),
        "M" => Duration::from_secs(number * 60),
        "S" => Duration::from_secs(number),
        "m" => Duration::from_millis(number),
        "u" => Duration::from_micros(number),
        "n" => Duration::from_nanos(number),
        _ => return Err(invalid()),
    };
    Instant::now()
        .checked_add(duration)
        .map(Some)
        .ok_or_else(invalid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Bytes;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use tower::ServiceExt;

    struct Active(Arc<AtomicUsize>);
    impl Active {
        fn new(count: &Arc<AtomicUsize>) -> Self {
            count.fetch_add(1, Ordering::SeqCst);
            Self(count.clone())
        }
    }
    impl Drop for Active {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }

    fn request(timeout: &str) -> Request<()> {
        Request::builder()
            .header("grpc-timeout", timeout)
            .body(())
            .unwrap()
    }

    #[tokio::test]
    async fn invalid_duplicate_and_expired_deadlines_never_call_application() {
        let calls = Arc::new(AtomicUsize::new(0));
        for (raw, expected) in [
            ("1x", tonic::Code::InvalidArgument),
            ("123456789m", tonic::Code::InvalidArgument),
            ("-1S", tonic::Code::InvalidArgument),
            ("", tonic::Code::InvalidArgument),
            ("0n", tonic::Code::DeadlineExceeded),
        ] {
            let count = calls.clone();
            let inner = tower::service_fn(move |_: Request<()>| {
                count.fetch_add(1, Ordering::SeqCst);
                async { Ok::<_, Infallible>(Response::new(Body::empty())) }
            });
            let response = Deadline::new(inner).oneshot(request(raw)).await.unwrap();
            assert_eq!(
                Status::from_header_map(response.headers()).unwrap().code(),
                expected
            );
        }
        let mut duplicate = request("1S");
        duplicate
            .headers_mut()
            .append("grpc-timeout", "2S".parse().unwrap());
        assert_eq!(
            parse(duplicate.headers()).unwrap_err().code(),
            tonic::Code::InvalidArgument
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        for unit in ["H", "M", "S", "m", "u", "n"] {
            assert!(
                parse(request(&format!("1{unit}")).headers())
                    .unwrap()
                    .is_some()
            );
        }
    }

    #[tokio::test]
    async fn request_deadline_drops_inflight_application_future() {
        let active = Arc::new(AtomicUsize::new(0));
        let count = active.clone();
        let inner = tower::service_fn(move |_: Request<()>| {
            let guard = Active::new(&count);
            async move {
                let _guard = guard;
                std::future::pending::<Result<Response<Body>, Infallible>>().await
            }
        });
        let response = tokio::time::timeout(
            Duration::from_secs(2),
            Deadline::new(inner).oneshot(request("30m")),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(
            Status::from_header_map(response.headers()).unwrap().code(),
            tonic::Code::DeadlineExceeded
        );
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn response_deadline_emits_terminal_trailer_and_drops_inner_body() {
        let active = Arc::new(AtomicUsize::new(0));
        let count = active.clone();
        let inner = tower::service_fn(move |_: Request<()>| {
            let guard = Active::new(&count);
            async move {
                let stream = async_stream::stream! {
                    let _guard=guard;
                    yield Ok::<_,Status>(Frame::data(Bytes::from_static(b"first")));
                    std::future::pending::<()>().await;
                };
                Ok::<_, Infallible>(Response::new(Body::new(StreamBody::new(stream))))
            }
        });
        let mut body = Deadline::new(inner)
            .oneshot(request("100m"))
            .await
            .unwrap()
            .into_body();
        assert_eq!(
            body.frame().await.unwrap().unwrap().into_data().unwrap(),
            "first"
        );
        assert_eq!(active.load(Ordering::SeqCst), 1);
        let terminal = tokio::time::timeout(Duration::from_secs(2), body.frame())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(
            Status::from_header_map(terminal.trailers_ref().unwrap())
                .unwrap()
                .code(),
            tonic::Code::DeadlineExceeded
        );
        assert_eq!(active.load(Ordering::SeqCst), 0);
        assert!(body.frame().await.is_none());
    }

    #[tokio::test]
    async fn completed_status_and_response_cancellation_preserve_resource_lifetime() {
        let inner = tower::service_fn(|_: Request<()>| async {
            Ok::<_, Infallible>(Status::not_found("业务事实不存在").into_http())
        });
        let response = Deadline::new(inner).oneshot(request("1S")).await.unwrap();
        assert_eq!(
            Status::from_header_map(response.headers()).unwrap().code(),
            tonic::Code::NotFound
        );
        let active = Arc::new(AtomicUsize::new(0));
        let count = active.clone();
        let inner = tower::service_fn(move |_: Request<()>| {
            let guard = Active::new(&count);
            async move {
                let stream = async_stream::stream! {
                    let _guard=guard;
                    yield Ok::<_,Status>(Frame::data(Bytes::from_static(b"first")));
                    std::future::pending::<()>().await;
                };
                Ok::<_, Infallible>(Response::new(Body::new(StreamBody::new(stream))))
            }
        });
        let mut body = Deadline::new(inner)
            .oneshot(request("1H"))
            .await
            .unwrap()
            .into_body();
        body.frame().await.unwrap().unwrap();
        assert_eq!(active.load(Ordering::SeqCst), 1);
        drop(body);
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }
}
