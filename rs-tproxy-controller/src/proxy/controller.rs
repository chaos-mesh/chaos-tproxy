use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::Result;
use http::{Method, Request};
use hyper::client::connect::{Connected, Connection};
use hyper::client::Client;
use hyper::service::Service;
use hyper::{Body, Uri};
use rs_tproxy_proxy::raw_config::RawConfig as ProxyRawConfig;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::UnixStream;

#[derive(Debug, Clone)]
struct UnixConnect(PathBuf);

#[derive(Debug)]
struct UnixConnection(UnixStream);

impl Service<Uri> for UnixConnect {
    type Response = UnixConnection;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn 'static + Send + Future<Output = Result<Self::Response>>>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, _: Uri) -> Self::Future {
        let path = self.0.clone();
        Box::pin(async move { Ok(UnixConnection(UnixStream::connect(path).await?)) })
    }
}

pub async fn send_config(path: impl Into<PathBuf>, config: &ProxyRawConfig) -> Result<()> {
    let client = Client::builder().build(UnixConnect(path.into()));
    let request = Request::builder()
        .uri("/")
        .method(Method::PUT)
        .body(Body::from(serde_json::to_vec(config)?))?;
    let resp = client.request(request).await?;
    if !resp.status().is_success() {
        return Err(anyhow::anyhow!(
            "fail to send config: status({})",
            resp.status()
        ));
    }
    Ok(())
}

impl Connection for UnixConnection {
    fn connected(&self) -> Connected {
        Connected::new()
    }
}

impl AsyncRead for UnixConnection {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl AsyncWrite for UnixConnection {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}
