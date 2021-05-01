mod accept;
mod stream;

use std::convert::{Infallible, TryInto};
use std::sync::Arc;
use std::task::{Context, Poll};

use anyhow::anyhow;
use futures::{pin_mut, select, FutureExt, TryStreamExt};
use http::{Method, Request, Response, StatusCode};
use hyper::service::Service;
use hyper::{Body, Server};
use tokio::sync::Mutex;
use tracing::instrument;

use self::accept::accept_std_stream;
use self::stream::StdStream;
use crate::server_helper::{BoxedSendFuture, ServeHandler};
use crate::tproxy::config::Config;
use crate::{tproxy, RawConfig};

pub struct ConfigServer {
    tproxy_server: Arc<Mutex<tproxy::HttpServer>>,
    handler: Option<ServeHandler>,
}

impl ConfigServer {
    pub fn watch(tproxy_server: tproxy::HttpServer) -> Self {
        Self {
            tproxy_server: Arc::new(Mutex::new(tproxy_server)),
            handler: None,
        }
    }

    pub async fn start(&mut self) -> anyhow::Result<()> {
        if self.handler.is_some() {
            return Err(anyhow!("there is already a config server running"));
        }

        let server = Server::builder(accept_std_stream())
            .http1_keepalive(true)
            .serve(ServerImpl(self.tproxy_server.clone()));
        self.handler = Some(ServeHandler::serve(move |rx| async move {
            let rx = rx.fuse();
            let server = server.fuse();
            pin_mut!(rx);
            pin_mut!(server);
            select! {
                signal = rx => signal.map_err::<anyhow::Error, _>(Into::into),
                ret = server => ret.map_err(Into::into),
            }
        }));
        Ok(())
    }

    pub async fn stop(&mut self) -> anyhow::Result<()> {
        self.tproxy_server.lock().await.stop().await?;
        match self.handler.take() {
            None => return Err(anyhow!("there is no config server running")),
            Some(handler) => handler.stop().await,
        }
    }
}

#[derive(Debug, Clone)]
struct ServerImpl(Arc<Mutex<tproxy::HttpServer>>);

impl ServerImpl {
    async fn read_config(request: Request<Body>) -> anyhow::Result<Config> {
        let request_data: Vec<u8> = request
            .into_body()
            .try_fold(vec![], |mut data, seg| {
                data.extend(seg);
                futures::future::ok(data)
            })
            .await?;

        let raw_config: RawConfig = serde_json::from_slice(&request_data)?;
        raw_config.try_into()
    }

    #[instrument]
    async fn handle(
        handler: &mut tproxy::HttpServer,
        request: Request<Body>,
    ) -> anyhow::Result<Response<Body>> {
        if request.method() != Method::PUT {
            return Ok(Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .body(Body::empty())?);
        }

        let config = Self::read_config(request).await;

        if let Err(err) = config {
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(err.to_string().into())?);
        }

        if let Err(err) = handler.reload(config.unwrap()).await {
            return Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(err.to_string().into())?);
        }

        Ok(Response::builder()
            .status(StatusCode::OK)
            .body(Body::empty())?)
    }
}

impl Service<&StdStream> for ServerImpl {
    type Response = Self;
    type Error = Infallible;
    type Future = BoxedSendFuture<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, _: &StdStream) -> Self::Future {
        let service = self.clone();
        Box::pin(async move { Ok(service) })
    }
}

impl Service<Request<Body>> for ServerImpl {
    type Response = Response<Body>;
    type Error = anyhow::Error;
    type Future = BoxedSendFuture<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let handler = self.0.clone();
        Box::pin(async move { Self::handle(&mut *handler.lock().await, request).await })
    }
}
