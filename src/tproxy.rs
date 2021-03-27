use std::future::Future;
use std::matches;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use anyhow::Error;
use config::Config;
use connector::HttpConnector;
use hyper::service::Service;
use hyper::{Body, Client, Request, Response};
use tokio::net::TcpStream;
use url::Url;

use crate::handler::{
    apply_request_action, apply_response_action, select_request, select_response, PacketTarget,
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
}

impl Service<&TcpStream> for HttpServer {
    type Response = HttpService;
    type Error = std::io::Error;
    type Future =
        Pin<Box<dyn 'static + Send + Future<Output = Result<Self::Response, Self::Error>>>>;

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
    type Future = impl Future<Output = Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    // TODO: support action chain
    // TODO: support selection by port
    // TODO: deal with thrown errors
    #[inline]
    fn call(&mut self, mut request: Request<Body>) -> Self::Future {
        let service = self.clone();
        async move {
            if matches!(service.config.handler_config.packet, PacketTarget::Request)
                && select_request(&request, &service.config.handler_config.selector)
            {
                request =
                    apply_request_action(request, &service.config.handler_config.action).await?;
            }

            let mut url: Url = request
                .uri()
                .to_string()
                .parse()
                .expect("fail to transfer url between crate http and crate rust-url");
            url.set_ip_host(service.target.ip()).unwrap();
            url.set_port(Some(service.target.port())).unwrap();
            *request.uri_mut() = url
                .to_string()
                .parse()
                .expect("fail to transfer url between crate http and crate rust-url");

            let uri_bak = request.uri().clone();
            let method_bak = request.method().clone();

            let mut respone = service.client.request(request).await.unwrap();
            if matches!(service.config.handler_config.packet, PacketTarget::Response)
                && select_response(
                    uri_bak,
                    method_bak,
                    &respone,
                    &service.config.handler_config.selector,
                )
            {
                respone =
                    apply_response_action(respone, &service.config.handler_config.action).await?;
            }
            Ok(respone)
        }
    }
}