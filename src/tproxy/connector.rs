use std::net::SocketAddr;
use std::sync::Arc;
use std::task::{Context, Poll};

use anyhow::{anyhow, Error, Result};
use http::uri::Scheme;
use http::Uri;
use hyper::client::connect::dns::GaiResolver;
use hyper::service::Service;
use tokio::net::{TcpSocket, TcpStream};

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
}

impl Service<Uri> for HttpConnector {
    type Response = TcpStream;
    type Error = Error;
    type Future = BoxedFuture<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, dst: Uri) -> Self::Future {
        let mut connector = self.clone();
        Box::pin(async move {
            let socket = TcpSocket::new_v4()?;
            socketopt::set_ip_transparent(&socket)?;
            socketopt::set_mark(&socket, connector.config.mark)?;
            socket.set_reuseaddr(true)?;
            Ok(socket.connect(connector.resolve(&dst).await?).await?)
        })
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
        addrs
            .next()
            .ok_or_else(|| anyhow!("cannot resolve {}", host))
    }
}
