//! tonic 会独立 spawn 连接；Host future 被取消或超时后仍须停止这些连接。
use crate::wait_stop;
use std::{
    future::Future,
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpStream,
    sync::watch,
};
use tonic::transport::server::Connected;

#[derive(Clone)]
pub(crate) struct Connections {
    stop: watch::Sender<bool>,
    active: watch::Sender<usize>,
}

impl Connections {
    pub(crate) fn new() -> Self {
        Self {
            stop: watch::channel(false).0,
            active: watch::channel(0).0,
        }
    }

    pub(crate) fn wrap(&self, stream: TcpStream) -> Connection {
        self.active.send_modify(|active| *active += 1);
        let mut stop = self.stop.subscribe();
        Connection {
            stream,
            stopped: Box::pin(async move { wait_stop(&mut stop).await }),
            aborted: false,
            active: self.active.clone(),
        }
    }

    pub(crate) fn abort(&self) {
        self.stop.send_replace(true);
    }

    pub(crate) async fn closed(&self) {
        let mut active = self.active.subscribe();
        loop {
            if *active.borrow_and_update() == 0 {
                return;
            }
            if active.changed().await.is_err() {
                return;
            }
        }
    }
}

pub(crate) struct Connection {
    stream: TcpStream,
    stopped: Pin<Box<dyn Future<Output = ()> + Send>>,
    aborted: bool,
    active: watch::Sender<usize>,
}

impl Connection {
    fn check(&mut self, cx: &mut Context<'_>) -> io::Result<()> {
        if self.aborted || self.stopped.as_mut().poll(cx).is_ready() {
            self.aborted = true;
            Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "Host 已停止 RPC 连接",
            ))
        } else {
            Ok(())
        }
    }
}

impl Connected for Connection {
    type ConnectInfo = <TcpStream as Connected>::ConnectInfo;
    fn connect_info(&self) -> Self::ConnectInfo {
        self.stream.connect_info()
    }
}

impl AsyncRead for Connection {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if let Err(error) = this.check(cx) {
            return Poll::Ready(Err(error));
        }
        Pin::new(&mut this.stream).poll_read(cx, buf)
    }
}

impl AsyncWrite for Connection {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        if let Err(error) = this.check(cx) {
            return Poll::Ready(Err(error));
        }
        Pin::new(&mut this.stream).poll_write(cx, buf)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        if let Err(error) = this.check(cx) {
            return Poll::Ready(Err(error));
        }
        Pin::new(&mut this.stream).poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.stream.is_write_vectored()
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if let Err(error) = this.check(cx) {
            return Poll::Ready(Err(error));
        }
        Pin::new(&mut this.stream).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_shutdown(cx)
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.active.send_modify(|active| *active -= 1);
    }
}
