use std::path::PathBuf;

use tokio::signal::unix::SignalKind;

use crate::controller::CtrlServer;
use crate::signal::Signals;

pub mod controller;
pub mod handler;
pub mod proxy;
pub mod raw_config;
pub mod signal;
pub mod task;

pub async fn proxy_main(path: PathBuf) -> anyhow::Result<()> {
    tracing::info!("proxy get uds path {:?}", path);
    let mut server = CtrlServer::build(path).await?;
    server.start().await?;
    let mut signals = Signals::from_kinds(&[SignalKind::interrupt(), SignalKind::terminate()])?;
    signals.wait().await?;
    server.stop().await
}
