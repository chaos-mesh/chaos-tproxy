pub mod cmd;
pub mod handler;
pub mod route;
pub mod tproxy;

use std::net::SocketAddr;

use anyhow::anyhow;
use cmd::get_config;
use futures::future::FutureExt;
use futures::{pin_mut, select};
use hyper::Server;
use route::set_all_routes;
use tokio::signal::ctrl_c;
use tokio::signal::unix::{signal, SignalKind};
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

    let route_guard = set_all_routes(cfg.clone())
        .map_err(|err| anyhow!("fail to set routes: {}", err.to_string()))?;

    let server = Server::builder(incoming).serve(HttpServer::new(cfg));
    info!("tproxy is running on {}", addr);

    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let graceful = server.with_graceful_shutdown(async {
        rx.await.ok();
    });

    tokio::spawn(async move {
        // Await the `server` receiving the signal...
        if let Err(e) = graceful.await {
            info!("server error: {}", e);
        }
    });

    let recv_sigint = ctrl_c().fuse();
    let mut sigterm = signal(SignalKind::terminate())?;
    let recv_sigterm = sigterm.recv().fuse();

    pin_mut!(recv_sigint);
    pin_mut!(recv_sigterm);

    select! {
        sigint = recv_sigint => sigint?,
        _ = recv_sigterm => (),
    };

    let _ = tx.send(());
    drop(route_guard);
    Ok(())
}
