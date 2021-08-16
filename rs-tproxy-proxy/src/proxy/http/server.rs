use std::future::Future;
use std::matches;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use anyhow::Result;
use http::uri::{Scheme, Uri};
use http::StatusCode;
use hyper::server::conn::Http;
use hyper::service::Service;
use hyper::{Body, Client, Request, Response};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::select;
use tokio::sync::oneshot::Receiver;
use tracing::{debug, error};

use crate::handler::http::action::{apply_request_action, apply_response_action};
use crate::handler::http::rule::Target;
use crate::handler::http::selector::{select_request, select_response};
use crate::proxy::http::config::Config;
use crate::proxy::http::connector::HttpConnector;
use crate::proxy::tcp::listener::TcpListener;
use crate::proxy::tcp::transparent_socket::TransparentSocket;

#[derive(Debug)]
pub struct HttpServer {
    config: Config,
}

impl HttpServer {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn serve(&mut self, rx: Receiver<()>) -> Result<()> {
        let addr = SocketAddr::from(([0, 0, 0, 0], self.config.proxy_port));
        let listener = TcpListener::bind(addr)?;
        select! {
            _ = async {
                loop {
                    let stream = listener.accept().await?;
                    let addr_remote = stream.peer_addr()?;
                    let addr_local = stream.local_addr()?;
                    let config = Arc::new(self.config.clone());
                    let service = HttpService::new(addr_remote,addr_local, config);
                    tokio::spawn(async move {
                        match serve_http_with_error_return(stream, &service).await{
                            Ok(_)=>{}
                            Err(e) => {tracing::error!("{}",e);}
                        };
                    });
                }
                #[allow(unreachable_code)]
                Ok::<_, anyhow::Error>(())
            } => {},
            _ = rx => {
                return Ok(());
            }
        };
        Ok(())
    }
}

pub async fn serve_http_with_error_return(
    mut stream: TcpStream,
    service: &HttpService,
) -> Result<()> {
    loop {
        let (r, parts) = Http::new()
            .error_return(true)
            .serve_connection_with_parts(stream, service.clone())
            .await;

        let part_stream = match r {
            Ok(()) => match parts {
                Some(part) => part.io,
                None => {
                    return Ok(());
                }
            },
            Err(e) => {
                return if e.is_parse() {
                    tracing::debug!("turn into tcp transfer");
                    match parts {
                        Some(mut part) => {
                            let addr_target = part.io.local_addr()?;
                            let addr_local = part.io.peer_addr()?;
                            let socket = TransparentSocket::bind(addr_local)?;
                            let mut client_stream = socket.connect(addr_target).await?;
                            println!("{:?}", part.read_buf.as_ref());
                            client_stream
                                .write_all(part.read_buf.as_ref())
                                .await
                                .unwrap();
                            tokio::io::copy_bidirectional(&mut part.io, &mut client_stream).await?;
                            Ok(())
                        }
                        None => Ok(()),
                    }
                } else {
                    error!("fail to serve http: {}", e);
                    Ok(())
                }
            }
        };
        stream = part_stream;
    }
}

#[derive(Debug, Clone)]
pub struct HttpService {
    remote: SocketAddr,
    target: SocketAddr,
    config: Arc<Config>,
}

impl HttpService {
    fn new(addr_remote: SocketAddr, addr_target: SocketAddr, config: Arc<Config>) -> Self {
        Self {
            remote: addr_remote,
            target: addr_target,
            config: config.clone(),
        }
    }

    async fn handle(self, mut request: Request<Body>) -> Result<Response<Body>> {
        debug!("Proxy is handling http request");
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

        parts.authority = match self.target.to_string().parse() {
            Ok(o) => Some(o),
            Err(_) => None,
        };
        if parts.path_and_query.is_some() && parts.authority.is_some() && parts.scheme.is_none() {
            parts.scheme = Some(Scheme::HTTP);
        }

        *request.uri_mut() = Uri::from_parts(parts)?;

        let client = Client::builder().build(HttpConnector::new(self.remote));
        let mut response = match client.request(request).await {
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

impl Service<Request<Body>> for HttpService {
    type Response = Response<Body>;
    type Error = anyhow::Error;
    type Future =
        Pin<Box<dyn 'static + Send + Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, request: Request<Body>) -> Self::Future {
        Box::pin(self.clone().handle(request))
    }
}
