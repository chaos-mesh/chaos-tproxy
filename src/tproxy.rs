use std::future::Future;
use std::matches;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use anyhow::{anyhow, Error, Result};
use config::Config;
use connector::HttpConnector;
use http::uri::{Scheme, Uri};
use http::StatusCode;
use hyper::service::Service;
use hyper::{Body, Client, Request, Response, Server};
use tokio::net::TcpStream;
use tokio::sync::oneshot::{channel, Sender};
use tokio::task::{spawn, spawn_blocking, JoinHandle};
use tracing::{debug, error, info, instrument};

use crate::handler::{
    apply_request_action, apply_response_action, select_request, select_response, Target,
};
use crate::route::{clear_routes, set_all_routes};

pub mod config;
pub mod connector;
pub mod listener;
pub mod socketopt;

pub use listener::TcpIncoming;
pub struct HttpServer {
    config: Config,
    handler: Option<ServeHandler>,
}

struct ServerImpl(Arc<Config>);

struct ServeHandler {
    sender: Sender<()>,
    handler: JoinHandle<()>,
}

#[derive(Debug, Clone)]
pub struct HttpService {
    target: SocketAddr,
    config: Arc<Config>,
    client: Arc<Client<HttpConnector>>,
}

impl ServeHandler {
    fn serve(server: Server<TcpIncoming, ServerImpl>) -> Self {
        let (sender, rx) = channel();
        let handler = spawn(async move {
            // Await the `server` receiving the signal...
            if let Err(e) = server
                .with_graceful_shutdown(async move {
                    rx.await.ok();
                })
                .await
            {
                info!("server error: {}", e);
            }
        });
        Self { sender, handler }
    }

    async fn stop(self) -> Result<()> {
        let ServeHandler { sender, handler } = self;
        let _ = sender.send(());
        let _ = handler.await?;
        Ok(())
    }
}

impl HttpServer {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            handler: None,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        if self.handler.is_some() {
            return Err(anyhow!("there is already a server running"));
        }

        let addr = SocketAddr::from(([0, 0, 0, 0], self.config.listen_port));
        let incoming = TcpIncoming::bind(addr, self.config.ignore_mark)?;
        self.config.listen_port = incoming.local_addr().port();

        let cfg = self.config.clone();
        spawn_blocking(move || {
            set_all_routes(&cfg).map_err(|err| anyhow!("fail to set routes: {}", err.to_string()))
        })
        .await??;

        let server = Server::builder(incoming).serve(ServerImpl(Arc::new(self.config.clone())));
        self.handler = Some(ServeHandler::serve(server));
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        match self.handler.take() {
            None => return Err(anyhow!("there is no server running")),
            Some(handler) => handler.stop().await?,
        }
        let cfg = self.config.clone();
        spawn_blocking(move || {
            clear_routes(&cfg).map_err(|err| anyhow!("fail to clear routes: {}", err.to_string()))
        })
        .await?
    }

    pub async fn reload(&mut self, config: Config) -> Result<()> {
        self.stop().await?;
        self.config = config;
        self.start().await
    }
}

impl HttpService {
    fn new(addr: SocketAddr, config: Arc<Config>) -> Self {
        Self {
            target: addr
                .to_string()
                .parse()
                .expect("socket addr must be valid authority"),
            config: config.clone(),
            client: Arc::new(Client::builder().build(HttpConnector::new(config))),
        }
    }

    async fn handle(self, mut request: Request<Body>) -> Result<Response<Body>> {
        if let Some(rule) = self.config.rules.iter().find(|rule| {
            matches!(rule.target, Target::Request)
                && select_request(self.target.port(), &request, &rule.selector)
        }) {
            debug!("request matched");
            request = apply_request_action(request, &rule.actions).await?;
        }

        let uri = request.uri().clone();
        let method = request.method().clone();
        let headers = request.headers().clone();

        let mut parts = request.uri().clone().into_parts();
        if parts.scheme.is_none() {
            parts.scheme = Some(Scheme::HTTP);
        }
        parts.authority = Some(self.target.to_string().parse()?);
        *request.uri_mut() = Uri::from_parts(parts)?;

        let mut response = match self.client.request(request).await {
            Ok(resp) => resp,
            Err(err) => {
                error!("fail to forward request: {}", err);
                Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .body(Body::empty())?
            }
        };

        if let Some(rule) = self.config.rules.iter().find(|rule| {
            matches!(rule.target, Target::Response)
                && select_response(
                    self.target.port(),
                    &uri,
                    &method,
                    &headers,
                    &response,
                    &rule.selector,
                )
        }) {
            debug!("response matched");
            response = apply_response_action(response, &rule.actions).await?;
        }
        Ok(response)
    }
}

type BoxedFuture<T, E> = Pin<Box<dyn 'static + Send + Future<Output = Result<T, E>>>>;

impl Service<&TcpStream> for ServerImpl {
    type Response = HttpService;
    type Error = std::io::Error;
    type Future = BoxedFuture<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, stream: &TcpStream) -> Self::Future {
        let addr_result = stream.local_addr();
        let config = self.0.clone();
        Box::pin(async move { Ok(HttpService::new(addr_result?, config)) })
    }
}

impl Service<Request<Body>> for HttpService {
    type Response = Response<Body>;
    type Error = Error;
    type Future = BoxedFuture<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[instrument]
    #[inline]
    fn call(&mut self, request: Request<Body>) -> Self::Future {
        Box::pin(self.clone().handle(request))
    }
}
