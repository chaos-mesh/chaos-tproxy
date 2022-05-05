use std::convert::TryInto;
use std::future::Future;
use std::matches;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use anyhow::{anyhow, Result};
use derivative::Derivative;
use http::header::HOST;
use http::uri::{PathAndQuery, Scheme, Uri};
use http::StatusCode;
use hyper::server::conn::Http;
use hyper::service::Service;
use hyper::{client, Body, Client, Request, Response};
use rustls::ClientConfig;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::select;
use tokio::sync::oneshot::Receiver;
use tokio_rustls::TlsAcceptor;
use tracing::{debug, error, trace};

use crate::handler::http::action::{apply_request_action, apply_response_action};
use crate::handler::http::rule::Target;
use crate::handler::http::selector::{select_request, select_response};
use crate::proxy::http::config::{Config, HTTPConfig};
use crate::proxy::http::connector::HttpConnector;
use crate::proxy::tcp::listener::TcpListener;
use crate::proxy::tcp::transparent_socket::TransparentSocket;

pub struct HttpServer {
    config: Config,
}

impl HttpServer {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn serve(&mut self, rx: Receiver<()>) -> Result<()> {
        let addr = SocketAddr::from(([0, 0, 0, 0], self.config.http_config.proxy_port));
        let listener = TcpListener::bind(addr)?;
        tracing::info!(target : "Proxy", "Listening");
        let http_config = Arc::new(self.config.http_config.clone());
        if let Some(tls_config) = &self.config.tls_config {
            let tls_client_config = Arc::new(tls_config.tls_client_config.clone());
            let tls_server_config = Arc::new(tls_config.tls_server_config.clone());
            select! {
                _ = async {
                    loop {
                            let stream = listener.accept().await?;
                            let addr_remote = stream.peer_addr()?;
                            let addr_local = stream.local_addr()?;
                            tracing::debug!(target : "Accept streaming", "remote={:?}, local={:?}",
                            addr_remote, addr_local);
                            let service = HttpService::new(addr_remote,
                            addr_local,
                            http_config.clone(),
                            Some(tls_client_config.clone()));
                            let acceptor = TlsAcceptor::from(tls_server_config.clone());
                            tokio::spawn(async move {
                            match serve_https(stream, &service, acceptor).await{
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
            return Ok(());
        }
        select! {
            _ = async {
                loop {
                    let stream = listener.accept().await?;
                    let addr_remote = stream.peer_addr()?;
                    let addr_local = stream.local_addr()?;
                    tracing::debug!(target : "Accept streaming", "remote={:?}, local={:?}", addr_remote, addr_local);
                    let service = HttpService::new(addr_remote, addr_local, http_config.clone(), None);
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

pub async fn serve_https(
    stream: TcpStream,
    service: &HttpService,
    acceptor: TlsAcceptor,
) -> Result<()> {
    let log_key = format!(
        "{{ peer={},local={} }}",
        stream.peer_addr()?,
        stream.local_addr()?
    );
    let mut tls_stream = acceptor.accept(stream).await?;
    loop {
        let (r, parts) = Http::new()
            .serve_connection_with_parts(tls_stream, service.clone())
            .await;
        let part_stream = match r {
            Ok(()) => match parts {
                Some(part) => part.io,
                None => {
                    return Ok(());
                }
            },
            Err(e) => {
                return Err(anyhow!("{}: stream block with error: {}", log_key, e));
            }
        };
        tls_stream = part_stream;
    }
}

pub async fn serve_http_with_error_return(
    mut stream: TcpStream,
    service: &HttpService,
) -> Result<()> {
    let log_key = format!(
        "{{ peer={},local={} }}",
        stream.peer_addr()?,
        stream.local_addr()?
    );
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
                    tracing::debug!("{}:Turn into tcp transfer.", log_key);
                    match parts {
                        Some(mut part) => {
                            let addr_target = part.io.local_addr()?;
                            let addr_local = part.io.peer_addr()?;
                            let socket = TransparentSocket::bind(addr_local)?;
                            tracing::debug!("{}:Bind local addrs.", log_key);
                            let mut client_stream = socket.connect(addr_target).await?;
                            tracing::debug!("{}:Connected target addrs.", log_key);
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
                    if !e.to_string().contains("error shutting down connection") {
                        tracing::info!("{}:fail to serve http: {}", log_key, e);
                    }
                    Ok(())
                }
            }
        };
        stream = part_stream;
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
#[derive(Clone)]
pub struct HttpService {
    remote: SocketAddr,
    target: SocketAddr,
    config: Arc<HTTPConfig>,

    #[derivative(Debug = "ignore")]
    tls_client_config: Option<Arc<ClientConfig>>,
}

impl HttpService {
    fn new(
        addr_remote: SocketAddr,
        addr_target: SocketAddr,
        config: Arc<HTTPConfig>,
        tls_client_config: Option<Arc<ClientConfig>>,
    ) -> Self {
        Self {
            remote: addr_remote,
            target: addr_target,
            config,
            tls_client_config,
        }
    }

    async fn handle(self, mut request: Request<Body>) -> Result<Response<Body>> {
        let log_key = format!("{{remote = {}, target = {} }}", self.remote, self.target);
        debug!("{} : Proxy is handling http request", log_key);
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
            debug!("{} : request matched, rule({:?})", log_key, rule);
            request = apply_request_action(request, &rule.actions).await?;
        }

        let uri = request.uri().clone();
        let method = request.method().clone();
        let headers = request.headers().clone();
        trace!("URI: {}", request.uri());
        let mut parts = request.uri().clone().into_parts();

        parts.authority = match request
            .headers()
            .iter()
            .find(|(header_name, _)| **header_name == HOST)
        {
            None => match self.target.to_string().parse() {
                Ok(o) => Some(o),
                Err(_) => None,
            },
            Some((_, value)) => Some(value.as_bytes().try_into()?),
        };
        trace!("authority: {:?}", parts.authority);
        if parts.path_and_query.is_none() {
            parts.path_and_query = Some(PathAndQuery::from_static("/"))
        }
        if self.tls_client_config.is_some() {
            parts.scheme = Some(Scheme::HTTPS);
        } else {
            parts.scheme = Some(Scheme::HTTP);
        }

        *request.uri_mut() = Uri::from_parts(parts)?;

        let mut response = if let Some(tls_client_config) = &self.tls_client_config {
            let https = hyper_rustls::HttpsConnectorBuilder::new()
                .with_tls_config((**tls_client_config).clone())
                .https_only()
                .enable_http1()
                .enable_http2()
                .wrap_connector(HttpConnector::new(self.remote));

            let client: client::Client<_, hyper::Body> = client::Client::builder().build(https);
            match client.request(request).await {
                Ok(resp) => resp,
                Err(err) => {
                    error!("{} : fail to forward request: {}", log_key, err);
                    Response::builder()
                        .status(StatusCode::BAD_GATEWAY)
                        .body(Body::empty())?
                }
            }
        } else {
            let client = Client::builder().build(HttpConnector::new(self.remote));
            match client.request(request).await {
                Ok(resp) => resp,
                Err(err) => {
                    error!("{} : fail to forward request: {}", log_key, err);
                    Response::builder()
                        .status(StatusCode::BAD_GATEWAY)
                        .body(Body::empty())?
                }
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
            debug!("{} : response matched", log_key);
            response = apply_response_action(response, &rule.actions).await?;
        }
        Ok(response)
    }
}

impl Service<Request<Body>> for HttpService {
    type Response = Response<Body>;
    type Error = anyhow::Error;
    #[allow(clippy::type_complexity)]
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

#[test]
fn test_req() {
    let mut req = http::request::Request::new("hello world");
    (*req.headers_mut()).insert("Host", "earth".parse().unwrap());
    let mut parts = Uri::default().into_parts();

    parts.authority = match req
        .headers()
        .iter()
        .find(|(header_name, _)| **header_name == HOST)
    {
        None => None,
        Some((_, value)) => Some(value.as_bytes().try_into().unwrap()),
    };
    dbg!(parts.authority);
}
