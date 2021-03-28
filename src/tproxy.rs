use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use anyhow::{Error, Result};
use config::Config;
use connector::HttpConnector;
use http::uri::{Scheme, Uri};
use hyper::service::Service;
use hyper::{Body, Client, Request, Response};
use tokio::net::TcpStream;
use tracing::{debug, instrument};

use crate::handler::{
    apply_request_action, apply_response_action, select_request, select_response,
};

pub mod config;
pub mod connector;
pub mod listener;
pub mod socketopt;

pub use listener::TcpIncoming;
pub struct HttpServer {
    config: Arc<Config>,
}

#[derive(Debug, Clone)]
pub struct HttpService {
    target: SocketAddr,
    config: Arc<Config>,
    client: Arc<Client<HttpConnector>>,
}

impl HttpServer {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
        }
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

    // TODO: support selection by port
    // TODO: deal with thrown errors
    async fn handle(self, mut request: Request<Body>) -> Result<Response<Body>> {
        if let Some(rule) = self
            .config
            .rules
            .request
            .iter()
            .find(|rule| select_request(&request, &rule.selector))
        {
            debug!("request matched");
            request = apply_request_action(request, &rule.action).await?;
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

        let mut response = self.client.request(request).await.unwrap();
        if let Some(rule) = self
            .config
            .rules
            .response
            .iter()
            .find(|rule| select_response(&uri, &method, &headers, &response, &rule.selector))
        {
            debug!("response matched");
            response = apply_response_action(response, &rule.action).await?;
        }
        Ok(response)
    }
}

type BoxedFuture<T, E> = Pin<Box<dyn 'static + Send + Future<Output = Result<T, E>>>>;

impl Service<&TcpStream> for HttpServer {
    type Response = HttpService;
    type Error = std::io::Error;
    type Future = BoxedFuture<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, stream: &TcpStream) -> Self::Future {
        let addr_result = stream.local_addr();
        let config = self.config.clone();
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
