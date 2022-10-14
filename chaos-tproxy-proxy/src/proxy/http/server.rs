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
use tracing::{debug, error, span, trace, Level};

use crate::handler::http::action::{apply_request_action, apply_response_action};
use crate::handler::http::rule::Target;
use crate::handler::http::selector::{select_request, select_response, select_role};
use crate::proxy::http::config::{Config, HTTPConfig};
use crate::proxy::http::connector::HttpConnector;
use crate::proxy::tcp::listener::TcpListener;
use crate::proxy::tcp::transparent_socket::TransparentSocket;

/// HttpServer is the proxy service behind the iptables tproxy. It would accept the forwarded
/// connection from the iptables tproxy, and then let [HttpService] to handle the connection.
pub struct HttpServer {
    config: Config,
}

impl HttpServer {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn serve(&mut self, mut rx: Receiver<()>) -> Result<()> {
        let addr = SocketAddr::from(([0, 0, 0, 0], self.config.http_config.proxy_port));
        let listener = TcpListener::bind(addr)?;
        tracing::info!("Proxy Listening");
        let http_config = Arc::new(self.config.http_config.clone());
        let rx_mut = &mut rx;

        loop {
            let stream = select! {
                stream = listener.accept() => {
                    stream
                },
                _ = &mut *rx_mut => {
                    return Ok(());
                }
            }?;
            let addr_remote = stream.peer_addr()?;
            let addr_local = stream.local_addr()?;
            debug!(target : "Accept streaming", "remote={:?}, local={:?}",addr_remote, addr_local);
            if let Some(tls_config) = &self.config.tls_config {
                let tls_client_config = Arc::new(tls_config.tls_client_config.clone());
                let tls_server_config = Arc::new(tls_config.tls_server_config.clone());
                let service = HttpService::new(
                    addr_remote,
                    addr_local,
                    http_config.clone(),
                    Some(tls_client_config.clone()),
                );
                let acceptor = TlsAcceptor::from(tls_server_config.clone());
                tokio::spawn(async move {
                    match serve_https(stream, &service, acceptor).await {
                        Ok(_) => {}
                        Err(e) => {
                            error!("{}", e);
                        }
                    };
                });
            } else {
                let service = HttpService::new(addr_remote, addr_local, http_config.clone(), None);
                tokio::spawn(async move {
                    match serve_http_with_error_return(stream, &service).await {
                        Ok(_) => {}
                        Err(e) => {
                            error!("{}", e);
                        }
                    };
                });
            }
        }
    }
}

/// serve_https would make the HttpService resolving the resolve TLS stream.
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
                // TODO(@STRRL): require similar error resolving in `serve_http_with_error_return`
                return Err(anyhow!("{}: stream block with error: {}", log_key, e));
            }
        };
        tls_stream = part_stream;
    }
}

///  serve_http_with_error_return would make the HttpService resolve the incoming TCP stream.
///
/// TODO(@STRRL): rename it to `serve_http` to keep naming consistent with `serve_https`
pub async fn serve_http_with_error_return(
    mut stream: TcpStream,
    service: &HttpService,
) -> Result<()> {
    let log_key = format!(
        "{{ peer={},local={} }}",
        stream.peer_addr()?,
        stream.local_addr()?
    );
    let span = span!(Level::TRACE, "Stream", "{}", &log_key);
    let _guard = span.enter();
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
                    debug!("Turn into tcp transfer.");
                    match parts {
                        Some(mut part) => {
                            let addr_target = part.io.local_addr()?;
                            let addr_local = part.io.peer_addr()?;
                            let socket = TransparentSocket::bind(addr_local)?;
                            debug!("Bind local addrs.");
                            let mut client_stream = socket.connect(addr_target).await?;
                            debug!("Connected target addrs.");
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
                        tracing::info!("fail to serve http: {}", e);
                    }
                    Ok(())
                }
            }
        };
        stream = part_stream;
    }
}

/// HttpService could handle the forwarded connection from [HttpServer], it would parse the packet
/// content, forwarding the request to the target server, and then return the response to the client.
/// Also, it would inject the chaos at the same time.
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

    /// role_ok would check the role of the chaos-tproxy, eg. working on client-side or server-side.
    /// If `role` in config is `None`, it would effect both client-side and server-side.
    fn role_ok(&self) -> bool {
        let role = match &self.config.role {
            None => return true,
            Some(r) => r.clone(),
        };

        select_role(&self.remote.ip(), &self.target.ip(), &role)
    }

    /// handle would execute the core inject and forward logic.
    async fn handle(self, mut request: Request<Body>) -> Result<Response<Body>> {
        let log_key = format!("{{remote = {}, target = {} }}", self.remote, self.target);
        debug!("{} : Proxy is handling http request", log_key);

        let role_ok = self.role_ok();
        let request_rules: Vec<_> = self
            .config
            .rules
            .iter()
            .filter(|rule| {
                role_ok
                    && matches!(rule.target, Target::Request)
                    && select_request(self.target.port(), &request, &rule.selector)
            })
            .collect();

        // inject chaos into request
        for rule in request_rules {
            debug!("{} : request matched, rule({:?})", log_key, rule);
            request = apply_request_action(request, &rule.actions).await?;
        }

        let uri = request.uri().clone();
        let method = request.method().clone();
        let headers = request.headers().clone();
        trace!("URI: {}", request.uri());
        let mut parts = request.uri().clone().into_parts();

        // because the original request URL is not carried in the HTTP request, we should rebuild it.
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

        // forward HTTP/HTTPS request
        let rsp_fut = if let Some(tls_client_config) = &self.tls_client_config {
            let https = hyper_rustls::HttpsConnectorBuilder::new()
                .with_tls_config((**tls_client_config).clone())
                .https_only()
                .enable_http1()
                .enable_http2()
                .wrap_connector(HttpConnector::new(self.target, self.remote));

            let client: client::Client<_, hyper::Body> = client::Client::builder().build(https);
            client.request(request)
        } else {
            let client = Client::builder().build(HttpConnector::new(self.target, self.remote));
            client.request(request)
        };

        let mut response = match rsp_fut.await {
            Ok(resp) => resp,
            Err(err) => {
                error!("{} : fail to forward request: {}", log_key, err);
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
                role_ok
                    && matches!(rule.target, Target::Response)
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

        // inject chaos into response
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
