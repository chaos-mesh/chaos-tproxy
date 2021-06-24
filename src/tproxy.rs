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
use tokio::net::{TcpStream, TcpSocket};
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
use hyper::server::accept::Accept;
use futures::{Future, TryFuture};
use std::pin::Pin;
use hyper::server::conn::{Http, Parts};
use hyper::body::Bytes;
use tokio::io::{AsyncWriteExt, AsyncWrite};
use tokio::sync::oneshot::error::RecvError;
use tokio::sync::oneshot::Receiver;
use std::intrinsics::floorf32;

#[derive(Debug)]
pub struct HttpServer {
    config: Config,
    rx: Option<Receiver<()>>,
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
            rx: None,
            handler: None
        }
    }

    pub async fn reload(&mut self, config: Config) -> Result<()> {
        self.stop().await?;
        self.config = config;
        self.start().await
    }
}

impl Future for HttpServer {
    type Output = Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.handler.is_some() {
            return Poll::Ready(Err(anyhow!("there is already a tproxy server running")));
        }

        let addr = SocketAddr::from(([0, 0, 0, 0], self.config.listen_port));
        let incoming = TcpIncoming::bind(addr, self.config.ignore_mark)?;
        self.config.listen_port = incoming.local_addr().port();
        let cfg = self.config.clone();
        spawn_blocking(move || {
            set_all_routes(&cfg).map_err(|err| anyhow!("fail to set routes: {}", err.to_string()))
        }).await??;
        let mut service = ServerImpl(Arc::new(self.config.clone()));

        loop {
            match service.poll_ready() {
                Poll::Ready(_) => {},
                Poll::Pending => continue,
            };

            if let Poll::Ready(Some(item)) = incoming.poll_accept(cx) {
                let mut io = item.map_err(Err(anyhow!("new accept")))?;
                let mut connection = service.call(&io).await?;
                if self.rx.unwrap().poll(cx).is_ready() {
                    io.shutdown().await?;
                    return Poll::Ready(Ok(()));
                }
                tokio::spawn( async move {
                    loop {
                        let (io_, connection_) = {
                        let (r, o) = Http::new()
                            .error_return(true)
                            .serve_connection_with_error_return(io,connection)
                            .await;
                        match r {
                            Ok(o) => {
                                (o.io, o.service)
                            }
                            Err(e) => {
                                match o {
                                    None => { return ;}
                                    Some(p) => {
                                        let socket = TcpSocket::new_v4()?;
                                        let cf = socket.connect(p.io.peer_addr().unwrap())?;
                                        cf.write_all(p.read_buf.as_ref()).await;
                                        tokio::io::copy_bidirectional(p.io.into(), cf).await.map_err(
                                            |e| return
                                        );
                                        return ;
                                    }
                                }
                            }
                        }
                    };
                        io = io_;
                        connection = connection_;
                    }
                });
            };
        }
    }
}


#[async_trait]
impl SuperServer for HttpServer {
    async fn start(&mut self) -> Result<()> {
        if self.handler.is_some() {
            return Err(anyhow!("there is already a tproxy server running"));
        }

        self.handler = Some(ServeHandler::serve(move |rx| {
            self.rx = Some(rx);
            self.await;
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
