use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::{Error, Result};
use http::Uri;
use hyper::service::Service;
use tokio::net::TcpStream;
use tracing::{instrument, trace};

use crate::proxy::tcp::transparent_socket::TransparentSocket;

#[derive(Debug, Clone)]
pub struct HttpConnector {
    target: SocketAddr,
    socket: TransparentSocket,
}

impl HttpConnector {
    pub fn new(dst: SocketAddr, src: SocketAddr) -> Self {
        Self {
            target: dst,
            socket: TransparentSocket::new(src),
        }
    }

    async fn connect(self, _: Uri) -> Result<TcpStream> {
        Ok(self.socket.conn(self.target).await?)
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
        trace!("connector is ready");
        Poll::Ready(Ok(()))
    }

    #[instrument]
    fn call(&mut self, dst: Uri) -> Self::Future {
        Box::pin(self.clone().connect(dst))
    }
}
