use std::net::SocketAddr;
use std::sync::Arc;
use std::task::{Context, Poll};

use anyhow::{anyhow, Error, Result};
use http::uri::Scheme;
use http::Uri;
use hyper::client::connect::dns::GaiResolver;
use hyper::service::Service;
use tokio::net::{TcpSocket, TcpStream};
use tracing::{instrument, trace};

use super::config::Config;
use super::{socketopt, BoxedFuture};

#[derive(Debug, Clone)]
pub struct HttpConnector {
    config: Arc<Config>,
    resolver: GaiResolver,
}

impl HttpConnector {
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            config,
            resolver: GaiResolver::new(),
        }
    }

    async fn connect(mut self, dst: Uri) -> Result<TcpStream> {
        let socket = TcpSocket::new_v4()?;
        socketopt::set_ip_transparent(&socket)?;
        socketopt::set_mark(&socket, self.config.mark)?;
        socket.set_reuseaddr(true)?;
        let addr = self.resolve(&dst).await?;
        trace!("resolved addr({})", addr);
        Ok(socket.connect(addr).await?)
    }
}

impl Service<Uri> for HttpConnector {
    type Response = TcpStream;
    type Error = Error;
    type Future = BoxedFuture<Self::Response, Self::Error>;

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

impl HttpConnector {
    async fn resolve(&mut self, dst: &Uri) -> Result<SocketAddr> {
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
        let mut addrs = self.resolver.call(host.parse()?).await?;
        let mut addr = addrs
            .next()
            .ok_or_else(|| anyhow!("cannot resolve {}", host))?;

        if let Some(port) = dst.port() {
            addr.set_port(port.as_u16());
        }
        Ok(addr)
    }
}
