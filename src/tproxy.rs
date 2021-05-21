use std::matches;
use std::net::SocketAddr;
use std::sync::Arc;
use std::task::{Context, Poll};

use anyhow::{anyhow, Error, Result};
use async_trait::async_trait;
use connector::HttpConnector;
use http::uri::{Scheme, Uri};
use http::StatusCode;
use hyper::service::Service;
use hyper::{Body, Client, Request, Response, Server};
use tokio::net::TcpStream;
use tokio::task::spawn_blocking;
use tracing::{debug, error, instrument};

use self::config::Config;
use crate::handler::{
    apply_request_action, apply_response_action, select_request, select_response, Target,
};
use crate::route::{clear_routes, set_all_routes};
use crate::server_helper::{BoxedSendFuture, ServeHandler, SuperServer};

pub mod config;
pub mod connector;
pub mod listener;
pub mod socketopt;

pub use listener::TcpIncoming;

#[derive(Debug)]
pub struct HttpServer {
    config: Config,
    handler: Option<ServeHandler>,
}

struct ServerImpl(Arc<Config>);

#[derive(Debug, Clone)]
pub struct HttpService {
    target: SocketAddr,
    config: Arc<Config>,
    client: Arc<Client<HttpConnector>>,
}

impl HttpServer {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            handler: None,
        }
    }

    pub async fn reload(&mut self, config: Config) -> Result<()> {
        self.stop().await?;
        self.config = config;
        self.start().await
    }
}

#[async_trait]
impl SuperServer for HttpServer {
    async fn start(&mut self) -> Result<()> {
        if self.handler.is_some() {
            return Err(anyhow!("there is already a tproxy server running"));
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
        self.handler = Some(ServeHandler::serve(move |rx| {
            server.with_graceful_shutdown(async move {
                rx.await.ok();
            })
        }));
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        match self.handler.take() {
            None => return Err(anyhow!("there is no tproxy server running")),
            Some(handler) => handler.stop().await?,
        }
        let cfg = self.config.clone();
        spawn_blocking(move || {
            clear_routes(&cfg).map_err(|err| anyhow!("fail to clear routes: {}", err.to_string()))
        })
        .await?
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
        let request_rules: Vec<_> = self
            .config
            .rules
            .iter()
            .filter(|rule| {
                matches!(rule.target, Target::Request)
                    && select_request(self.target.port(), &request, &rule.selector)
            })
            .collect();

        for rule in request_rules {
            debug!("request matched, rule({:?})", rule);
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

        let response_rules: Vec<_> = self
            .config
            .rules
            .iter()
            .filter(|rule| {
                matches!(rule.target, Target::Response)
                    && select_response(
                        self.target.port(),
                        &uri,
                        &method,
                        &headers,
                        &response,
                        &rule.selector,
                    )
            })
            .collect();

        for rule in response_rules {
            debug!("response matched");
            response = apply_response_action(response, &rule.actions).await?;
        }
        Ok(response)
    }
}

impl Service<&TcpStream> for ServerImpl {
    type Response = HttpService;
    type Error = std::io::Error;
    type Future = BoxedSendFuture<Self::Response, Self::Error>;

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
    type Future = BoxedSendFuture<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[instrument]
    #[inline]
    fn call(&mut self, request: Request<Body>) -> Self::Future {
        Box::pin(self.clone().handle(request))
    }
}
