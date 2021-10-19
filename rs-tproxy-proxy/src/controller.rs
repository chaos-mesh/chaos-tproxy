use std::collections::HashMap;
use std::convert::TryInto;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use anyhow::Result;
use derive_more::{Deref, DerefMut};
use futures::TryStreamExt;
use http::{Method, Request, Response, StatusCode};
use hyper::server::conn::Http;
use hyper::service::Service;
use hyper::Body;
use tokio::fs::{metadata, read, read_dir};
use tokio::net::UnixListener;
use tokio::sync::Mutex;
use tracing::{debug, instrument, trace};

use super::handler::http::plugin::Plugin;
use super::proxy::http::config::Config;
use super::proxy::http::server::HttpServer;
use super::raw_config::RawConfig;
use super::task::Task;

const WASM_EXT: &str = ".wasm";

#[derive(Debug)]
pub struct CtrlServer {
    uds_path: PathBuf,
    service: CtrlService,
    task: Option<Task<()>>,
}

impl CtrlServer {
    pub async fn build(path: impl Into<PathBuf>) -> Result<Self> {
        Ok(Self {
            uds_path: path.into(),
            service: Default::default(),
            task: None,
        })
    }

    pub async fn start(&mut self) -> Result<()> {
        self.stop().await?;
        let service = self.service.clone();
        let uds_path = self.uds_path.clone();
        self.task = Some(Task::start(async move {
            let listener = UnixListener::bind(&uds_path)?;
            service.serve(listener).await
        }));
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        if let Some(task) = self.task.take() {
            task.stop().await?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CtrlService(Arc<Mutex<Option<ProxyGuard>>>);

#[derive(Debug, Default, Clone, Deref, DerefMut)]
pub struct PluginMap(HashMap<String, Plugin>);

#[derive(Debug)]
struct ProxyGuard {
    plugin_map: PluginMap,
    task: Option<Task<()>>,
}

impl ProxyGuard {
    fn start(config: Config, plugin_map: PluginMap) -> Self {
        let proxy = HttpServer::new(config, plugin_map.clone());
        Self {
            plugin_map,
            task: Some(Task::start(async move {
                tracing::info!("proxy starting");
                proxy.serve().await
            })),
        }
    }

    async fn stop(self) -> Result<PluginMap> {
        if let Some(task) = self.task {
            task.stop().await?;
        }
        Ok(self.plugin_map)
    }
}

impl PluginMap {
    pub fn must_get(&self, name: &str) -> Result<&Plugin> {
        self.get(name)
            .ok_or_else(|| anyhow::anyhow!("plugin `{}` not found", name))
    }

    async fn load_plugins(&mut self, plugin_path: &str) -> Result<()> {
        match metadata(plugin_path).await {
            Ok(meta) if meta.is_dir() => (),
            _ => return Ok(()),
        }

        debug!("ready to load plugins in path({})", plugin_path);
        let mut dir = read_dir(&plugin_path).await?;
        while let Some(entry) = dir.next_entry().await? {
            trace!("read entry: {:?}", entry);
            if !entry.file_type().await?.is_dir()
                && entry.file_name().to_string_lossy().ends_with(WASM_EXT)
            {
                debug!(
                    "ready to load plugin file({})",
                    entry.file_name().to_string_lossy()
                );
                let module = read(entry.path()).await?;
                let name = entry
                    .file_name()
                    .to_string_lossy()
                    .trim_end_matches(WASM_EXT)
                    .to_owned();
                match self.get_mut(&name) {
                    None => {
                        debug!("ready to load new plugin({})", name);
                        self.insert(name, Plugin::wasm(&module)?);
                    }
                    Some(plugin) => {
                        if plugin.is_change(&module) {
                            debug!("ready to update plugin({})", name);
                            *plugin = Plugin::wasm(&module)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl CtrlService {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }

    pub async fn serve(&self, listener: UnixListener) -> Result<()> {
        tracing::info!("controller listening");
        loop {
            let service = self.clone();
            let (stream, addr) = listener.accept().await?;
            tracing::debug!("accept streaming: addr={:?}", addr);
            tokio::spawn(async move {
                if let Err(err) = Http::new().serve_connection(stream, service).await {
                    tracing::error!("{}", err);
                }
            });
        }
    }

    async fn read_config(request: Request<Body>) -> Result<Config> {
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
    async fn handle(self, request: Request<Body>) -> anyhow::Result<Response<Body>> {
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
        debug!("read config: {:?}", config);
        let mut proxy = self.0.lock().await;
        let mut plugin_map = match proxy.take() {
            Some(proxy) => proxy.stop().await?,
            None => Default::default(),
        };
        debug!("ready to load plugins: current({:?})", plugin_map);
        plugin_map.load_plugins(&config.plugin_path).await?;
        debug!("plugins loaded: current({:?})", plugin_map);
        *proxy = Some(ProxyGuard::start(config, plugin_map));
        Ok(Response::builder()
            .status(StatusCode::OK)
            .body(Body::empty())?)
    }
}

impl Default for CtrlService {
    fn default() -> Self {
        Self::new()
    }
}

impl Service<Request<Body>> for CtrlService {
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
        Box::pin(Self::handle(self.clone(), request))
    }
}
