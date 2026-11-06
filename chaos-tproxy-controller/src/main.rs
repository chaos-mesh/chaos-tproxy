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
            if let Some(error) = e.downcast_ref::<clap::Error>() {
                error.exit();
            }
            eprintln!("{}", e);
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
        let mut signals = Signals::from_kinds(&[SignalKind::interrupt(), SignalKind::terminate()])?;
        proxy.reload(cfg.proxy_config).await?;
        signals.wait().await?;
        proxy.stop().await?;
        return Ok(());
    }

    if opt.interactive {
        let mut config_server = ConfigServer::new(Proxy::new(opt.verbose).await);
        let mut signals = Signals::from_kinds(&[SignalKind::interrupt(), SignalKind::terminate()])?;
        config_server.serve_interactive();

        let wait_result = tokio::select! {
            result = signals.wait() => result,
            result = config_server.wait() => result,
        };
        config_server.stop().await?;
        wait_result?;

        // Tokio cannot cancel a pending blocking stdin read during runtime shutdown.
        exit(0);
    }
    Ok(())
}
