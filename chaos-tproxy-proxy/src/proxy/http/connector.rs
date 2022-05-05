use std::convert::TryFrom;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::{anyhow, Error, Result};
use derivative::Derivative;
use http::Uri;
use hyper::client::connect::dns::GaiResolver;
use hyper::service::Service;
use rustls::ServerName;
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;
use tokio_rustls::TlsConnector;
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
    #[allow(clippy::type_complexity)]
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

#[derive(Derivative)]
#[derivative(Debug)]
#[derive(Clone)]
pub struct HttpsConnector {
    resolver: GaiResolver,
    socket: TransparentSocket,

    #[derivative(Debug = "ignore")]
    tls_conn: TlsConnector,
}

impl HttpsConnector {
    pub fn new(src: SocketAddr, tls_conn: TlsConnector) -> Self {
        Self {
            resolver: GaiResolver::new(),
            socket: TransparentSocket::new(src),
            tls_conn,
        }
    }

    async fn connect(mut self, dist: Uri) -> Result<TlsStream<TcpStream>> {
        let addr = resolve(&mut self.resolver, &dist).await?;
        trace!("resolved addr({})", dist);
        let stream = self.socket.conn(addr).await?;
        let addr_string = addr.to_string();
        let domain = dist.host().unwrap_or(&addr_string);
        trace!("domain name: {}", domain);
        let server_name = ServerName::try_from(domain)?;
        trace!("server_name: {:?}", server_name);
        Ok(self.tls_conn.connect(server_name, stream).await?)
    }
}

impl Service<Uri> for HttpsConnector {
    type Response = TlsStream<TcpStream>;
    type Error = Error;
    #[allow(clippy::type_complexity)]
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
