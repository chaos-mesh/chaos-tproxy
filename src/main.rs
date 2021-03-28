pub mod cmd;
pub mod handler;
pub mod route;
pub mod tproxy;

use std::net::SocketAddr;

use anyhow::anyhow;
use cmd::get_config;
use hyper::Server;
use route::set_all_routes;
use tproxy::{HttpServer, TcpIncoming};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init()
        .map_err(|err| anyhow!("{}", err))?;

    let mut cfg = get_config().await?;
    let addr = SocketAddr::from(([0, 0, 0, 0], cfg.listen_port));
    let incoming = TcpIncoming::bind(addr, cfg.ignore_mark)?;
    cfg.listen_port = incoming.local_addr().port();

    let _ = set_all_routes(cfg.clone())
        .map_err(|err| anyhow!("fail to set routes: {}", err.to_string()))?;

    let server = Server::builder(incoming).serve(HttpServer::new(cfg));
    info!("tproxy is running on {}", addr);
    server.await.map_err(Into::into)
}
