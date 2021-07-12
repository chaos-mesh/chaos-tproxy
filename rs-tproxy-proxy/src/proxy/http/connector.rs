use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::{anyhow, Error, Result};
use http::uri::Scheme;
use http::Uri;
use hyper::client::connect::dns::GaiResolver;
use hyper::service::Service;
use tokio::net::TcpStream;
use tracing::{instrument, trace};

use crate::proxy::tcp::transparent_socket::TransparentSocket;

#[derive(Debug, Clone)]
pub struct HttpConnector {
    resolver: GaiResolver,
    socket: TransparentSocket,
}

impl HttpConnector {
    pub fn new(src: SocketAddr) -> Self {
        Self {
            resolver: GaiResolver::new(),
            socket: TransparentSocket::new(src),
        }
    }

    async fn connect(mut self, dist: Uri) -> Result<TcpStream> {
        let addr = resolve(&mut self.resolver, &dist).await?;
        trace!("resolved addr({})", dist);
        Ok(self.socket.conn(addr).await?)
    }
}

impl Service<Uri> for HttpConnector {
    type Response = TcpStream;
    type Error = Error;
    type Future =
        Pin<Box<dyn 'static + Send + Future<Output = Result<Self::Response, Self::Error>>>>;

    #[instrument]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        futures::ready!(self.resolver.poll_ready(cx))?;
        trace!("connector is ready");
        Poll::Ready(Ok(()))
    }

    #[instrument]
    fn call(&mut self, dst: Uri) -> Self::Future {
        Box::pin(self.clone().connect(dst))
    }
}

/// This function resolve uri and select uri with scheme like `http://`
/// and get host addrs and dst port from Uri.
pub(crate) async fn resolve(resolver: &mut GaiResolver, dst: &Uri) -> Result<SocketAddr, Error> {
    if dst
        .scheme()
        .filter(|scheme| **scheme != Scheme::HTTP)
        .is_some()
    {
        return Err(anyhow!("http connector cannot handle http request"));
    }

    let host = dst
        .host()
        .ok_or_else(|| anyhow!("target uri has no host"))?;
    let mut addrs = resolver.call(host.parse()?).await?;
    let mut addr = addrs
        .next()
        .ok_or_else(|| anyhow!("cannot resolve {}", host))?;

    if let Some(port) = dst.port() {
        addr.set_port(port.as_u16());
    }
    Ok(addr)
}
