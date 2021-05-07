pub mod cmd;
pub mod config_server;
pub mod handler;
pub mod route;
pub mod server_helper;
pub mod signal;
pub mod tproxy;

use anyhow::anyhow;
use structopt::StructOpt;
use tokio::signal::unix::SignalKind;

use crate::cmd::config::RawConfig;
use crate::cmd::{get_config_from_opt, Opt};
use crate::config_server::ConfigServer;
use crate::signal::SignalHandler;
use crate::tproxy::HttpServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args_safe()?;
    tracing_subscriber::fmt()
        .with_max_level(opt.get_level_filter())
        .with_writer(std::io::stderr)
        .try_init()
        .map_err(|err| anyhow!("{}", err))?;

    let cfg = get_config_from_opt(opt).await?;
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
