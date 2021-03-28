#![feature(type_alias_impl_trait)]

pub mod cmd;
pub mod handler;
pub mod tproxy;

use std::net::SocketAddr;

use anyhow::anyhow;
use cmd::get_config;
use hyper::Server;
use tproxy::{HttpServer, TcpIncoming};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init()
        .map_err(|err| anyhow!("{}", err))?;

    let cfg = get_config().await?;
    let addr = SocketAddr::from(([0, 0, 0, 0], cfg.listen_port));
    let incoming = TcpIncoming::bind(addr, cfg.ignore_mark)?;
    let server = Server::builder(incoming).serve(HttpServer::new(cfg));
    info!("tproxy is running on {}", addr);
    server.await.map_err(Into::into)
}
