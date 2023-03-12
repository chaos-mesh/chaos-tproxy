use std::process::exit;

use chaos_tproxy_proxy::proxy_main;
use chaos_tproxy_proxy::signal::Signals;
use tokio::signal::unix::SignalKind;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

use crate::cmd::command_line::{get_config_from_opt, Opt};
use crate::cmd::interactive::handler::ConfigServer;
use crate::proxy::exec::Proxy;

pub mod cmd;
pub mod proxy;
pub mod raw_config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = match Opt::from_args_checked() {
        Err(e) => {
            println!("{}", e);
            exit(1)
        }
        Ok(o) => o,
    };
    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(EnvFilter::from_default_env().add_directive(opt.get_level_filter().into()))
        .with(EnvFilter::from_default_env().add_directive("chaos_tproxy".parse().unwrap()))
        .init();

    if opt.proxy {
        proxy_main(opt.ipc_path.clone().unwrap()).await?;
    }

    if opt.input.is_some() {
        let cfg = get_config_from_opt(&opt).await?;
        let mut proxy = Proxy::new(opt.verbose).await;
        proxy.reload(cfg.proxy_config).await?;
        let mut signals = Signals::from_kinds(&[SignalKind::interrupt(), SignalKind::terminate()])?;
        signals.wait().await?;
        proxy.stop().await?;
        return Ok(());
    }

    if let Some(path) = opt.interactive_path {
        let mut config_server = ConfigServer::new(Proxy::new(opt.verbose).await);
        config_server.serve_interactive(path.clone());

        let mut signals = Signals::from_kinds(&[SignalKind::interrupt(), SignalKind::terminate()])?;
        signals.wait().await?;
        // Currently we cannot graceful shutdown the config server.
        config_server.stop().await?;

        // delete the unix socket file
        std::fs::remove_file(path.clone())?;

        exit(0);
    }
    Ok(())
}
