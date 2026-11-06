use std::net::IpAddr;
use std::time::Duration;

use rand::random;
use surge_ping::{Client, Config, PingIdentifier, PingSequence};
use tokio::task::JoinHandle;
use tokio::time;

#[derive(Debug)]
pub(crate) struct GatewayPing {
    task: Option<JoinHandle<()>>,
}

impl GatewayPing {
    pub(crate) async fn start(addr: IpAddr) -> anyhow::Result<Self> {
        tracing::debug!("ping gateway {}", addr);
        let client = Client::new(&Config::default())?;
        let mut pinger = client.pinger(addr, PingIdentifier(random())).await;
        pinger.timeout(Duration::from_secs(1));
        let task = tokio::spawn(async move {
            // Pinger does not retain the client that owns its reply receiver.
            let _client = client;
            let mut sequence = 0u16;
            let mut interval = time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                sequence = sequence.wrapping_add(1);
                let _ = pinger.ping(PingSequence(sequence), &[0; 56]).await;
            }
        });
        Ok(Self { task: Some(task) })
    }

    pub(crate) async fn stop(&mut self) -> anyhow::Result<()> {
        if let Some(task) = self.task.as_mut() {
            task.abort();
            let result = task.await;
            self.task = None;
            match result {
                Ok(()) => (),
                Err(error) if error.is_cancelled() => (),
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }
}

impl Drop for GatewayPing {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
