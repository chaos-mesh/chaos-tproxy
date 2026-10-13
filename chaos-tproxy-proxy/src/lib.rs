use std::convert::TryInto;
use std::path::PathBuf;

use tokio::signal::unix::SignalKind;
use tokio::sync::oneshot::channel;

use crate::proxy::http::server::HttpServer;
use crate::signal::Signals;

pub mod bootstrap;
pub mod handler;
pub mod proxy;
pub mod raw_config;
pub mod signal;

pub async fn proxy_main(path: PathBuf) -> anyhow::Result<()> {
    tracing::info!("Proxy get uds path {:?}", path);
    let mut signals = Signals::from_kinds(&[SignalKind::interrupt(), SignalKind::terminate()])?;
    let (raw_config, completion) = bootstrap::receive(&path).await?;
    let mut server = match raw_config.try_into().and_then(HttpServer::bind) {
        Ok(server) => server,
        Err(error) => {
            if let Err(report_error) = completion.failed(&error).await {
                return Err(error.context(format!("report startup failure: {report_error:#}")));
            }
            return Err(error);
        }
    };
    completion.ready().await?;
    let (sender, rx) = channel();
    let serving = server.serve(rx);
    tokio::pin!(serving);
    tokio::select! {
        result = &mut serving => result,
        result = signals.wait() => {
            let _ = sender.send(());
            result?;
            serving.await
        }
    }
}
