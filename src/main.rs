#![feature(duration_constants)]
#![feature(type_alias_impl_trait)]

pub mod cmd;
pub mod config;
pub mod handler;
pub mod tproxy;

use std::net::SocketAddr;

use anyhow::anyhow;
use cmd::get_config;
use hyper::Server;
use tproxy::{HttpServer, TcpIncoming};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init()
        .map_err(|err| anyhow!("{}", err))?;

    let cfg = get_config().await?;
    let addr = SocketAddr::from(([0, 0, 0, 0], cfg.tproxy_config.port));
    let incoming = TcpIncoming::bind(addr, cfg.tproxy_config.mark)?;
    let server = Server::builder(incoming).serve(HttpServer::new(cfg.tproxy_config));
    server.await.map_err(Into::into)
}
