use std::path::PathBuf;
use std::convert::TryInto;

use tracing::trace;
use tokio::sync::oneshot::channel;

use crate::raw_config::RawConfig;
use crate::uds_client::UdsDataClient;
use crate::proxy::http::server::HttpServer;
use crate::signal::Signals;
use tokio::signal::unix::SignalKind;


pub mod uds_client;
pub mod raw_config;
pub mod proxy;
pub mod handler;
pub mod signal;

pub async fn proxy_main(path : PathBuf) -> anyhow::Result<()> {
    trace!("proxy get uds path :{:?}",path);
    let client = UdsDataClient::new(path);
    let mut buf: Vec<u8> = vec!();
    let raw_config: RawConfig = client.read_into(&mut buf).await?;
    let config = raw_config.try_into()?;
    let (sender, rx) = channel();

    let spawn = tokio::spawn(async move {
        let mut server = HttpServer::new(config);
        server.serve(rx).await.unwrap();
    });

    let mut signals =
        Signals::from_kinds(&[SignalKind::interrupt(), SignalKind::terminate()])?;
    signals.wait().await?;

    let _ = sender.send(());
    spawn.await?;
    Ok(())
}
