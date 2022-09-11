use std::convert::TryInto;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use anyhow::Error;
use futures::TryStreamExt;
use http::{Method, Request, Response, StatusCode};
use hyper::server::conn::Http;
use hyper::service::Service;
use hyper::Body;
use tokio::net::UnixListener;
use tokio::select;
use tokio::sync::oneshot::{channel, Receiver, Sender};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::instrument;

#[cfg(unix)]
use crate::proxy::config::Config;
use crate::proxy::exec::Proxy;
use crate::raw_config::RawConfig;

#[derive(Debug)]
pub struct ConfigServer {
    proxy: Arc<Mutex<Proxy>>,
    task: Option<JoinHandle<Result<(), Error>>>,
    rx: Option<Receiver<()>>,
    sender: Option<Sender<()>>,
}

impl ConfigServer {
    pub fn new(proxy: Proxy) -> Self {
        let (sender, rx) = channel();
        Self {
            proxy: Arc::new(Mutex::new(proxy)),
            task: None,
            rx: Some(rx),
            sender: Some(sender),
        }
    }

    pub fn serve_interactive(&mut self, interactive_path: PathBuf) {
        let mut rx = self.rx.take().unwrap();
        let mut service = ConfigService(self.proxy.clone());

        self.task = Some(tokio::spawn(async move {
            let rx_mut = &mut rx;
            tracing::info!("ConfigServer listener try binding {:?}", interactive_path);
            let unix_listener = UnixListener::bind(interactive_path).unwrap();

            loop {
                select! {
                    _ = &mut *rx_mut => {
                        tracing::trace!("catch signal in config server.");
                        return Ok(());
                    },
                    stream = unix_listener.accept() => {
                        let (stream, _) = stream.unwrap();

                        let http = Http::new();
                        let conn = http.serve_connection(stream, &mut service);
                        if let Err(e) = conn.await {
                            tracing::error!("{}",e);
                        }
                    },
                };
            }
        }));
    }

    pub async fn stop(mut self) -> anyhow::Result<()> {
        let mut proxy = self.proxy.lock().await;
        proxy.stop().await?;
        if let Some(sender) = self.sender.take() {
            let _ = sender.send(());
        };
        if let Some(handler) = self.task.take() {
            let _ = handler.await?;
        }
        Ok(())
    }
}

pub struct ConfigService(Arc<Mutex<Proxy>>);

impl ConfigService {
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
    async fn handle(proxy: &mut Proxy, request: Request<Body>) -> anyhow::Result<Response<Body>> {
        if request.method() != Method::PUT {
            return Ok(Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .body(Body::empty())?);
        }

        let config = match Self::read_config(request).await {
            Err(e) => {
                return Ok(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(e.to_string().into())?);
            }
            Ok(c) => c,
        };

        proxy.reload(config.proxy_config).await?;

        Ok(Response::builder()
            .status(StatusCode::OK)
            .body(Body::empty())?)
    }
}

impl Service<Request<Body>> for ConfigService {
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
        let handler = self.0.clone();
        Box::pin(async move { Self::handle(&mut *handler.lock().await, request).await })
    }
}
