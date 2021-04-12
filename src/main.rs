pub mod cmd;
pub mod handler;
pub mod route;
pub mod signal;
pub mod tproxy;

use std::convert::TryInto;

use anyhow::anyhow;
use cmd::config::RawConfig;
use cmd::get_config;
use futures::future::FutureExt;
use futures::{pin_mut, select};
use signal::SignalHandler;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::signal::unix::SignalKind;
use tproxy::HttpServer;
use tracing::error;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init()
        .map_err(|err| anyhow!("{}", err))?;

    let cfg = get_config().await?;
    let mut server = HttpServer::new(cfg);
    server.start().await?;

    let mut signal_handler =
        SignalHandler::from_kinds(&[SignalKind::interrupt(), SignalKind::terminate()])?;
    let signal_recv = signal_handler.wait().fuse();
    pin_mut!(signal_recv);

    let mut stdin = BufReader::new(tokio::io::stdin());
    loop {
        let mut buf = String::new();
        let read_line = stdin.read_line(&mut buf).fuse();
        pin_mut!(read_line);

        select! {
            _ = signal_recv => break,
            read_ret = read_line => {
                if let Err(err) = read_ret {
                    error!("error in receiving new config: {}", err);
                    break;
                }
                if let Err(err) = reload(&mut server, &buf).await {
                    error!("error in reloading http server: {}", err);
                    break;
                }
            }
        };
    }

    server.stop().await?;
    Ok(())
}

async fn reload(server: &mut HttpServer, buf: &str) -> anyhow::Result<()> {
    let cfg = serde_json::from_str::<RawConfig>(buf)?;
    server.reload(cfg.try_into()?).await?;
    Ok(())
}
