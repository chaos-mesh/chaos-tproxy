pub mod cmd;
pub mod config_server;
pub mod handler;
pub mod route;
pub mod server_helper;
pub mod signal;
pub mod tproxy;

use anyhow::anyhow;
use cmd::config::RawConfig;
use cmd::get_config;
use signal::SignalHandler;
use tokio::signal::unix::SignalKind;
use tracing_subscriber::EnvFilter;

use crate::config_server::ConfigServer;
use crate::tproxy::HttpServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .try_init()
        .map_err(|err| anyhow!("{}", err))?;

    let cfg = get_config().await?;
    let mut tproxy_server = HttpServer::new(cfg);
    tproxy_server.start().await?;

    let mut config_server = ConfigServer::watch(tproxy_server);
    config_server.start().await?;

    let mut signal_handler =
        SignalHandler::from_kinds(&[SignalKind::interrupt(), SignalKind::terminate()])?;
    signal_handler.wait().await?;
    config_server.stop().await?;
    Ok(())
}
