use std::convert::TryInto;
use std::path::PathBuf;

use tokio::signal::unix::SignalKind;
use tokio::sync::oneshot::channel;
use tracing::trace;

use crate::proxy::http::server::HttpServer;
use crate::raw_config::RawConfig;
use crate::signal::Signals;
use crate::uds_client::UdsDataClient;

pub mod handler;
pub mod proxy;
pub mod raw_config;
pub mod signal;
pub mod uds_client;

pub async fn proxy_main(path: PathBuf) -> anyhow::Result<()> {
    trace!("proxy get uds path :{:?}", path);
    let client = UdsDataClient::new(path);
    let mut buf: Vec<u8> = vec![];
    let raw_config: RawConfig = client.read_into(&mut buf).await?;
    let config = raw_config.try_into()?;
    let (sender, rx) = channel();

    let spawn = tokio::spawn(async move {
        let mut server = HttpServer::new(config);
        server.serve(rx).await.unwrap();
    });

    let mut signals = Signals::from_kinds(&[SignalKind::interrupt(), SignalKind::terminate()])?;
    signals.wait().await?;

    let _ = sender.send(());
    spawn.await?;
    Ok(())
}
