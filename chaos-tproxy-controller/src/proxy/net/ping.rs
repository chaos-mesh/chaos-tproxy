use std::net::IpAddr;
use std::time::Duration;

use rand::random;
use surge_ping::{Client, Config, PingIdentifier, PingSequence};
use tokio::time;

pub async fn try_ping(addr: IpAddr) {
    tracing::debug!("ping gateway {}", addr);
    let client = Client::new(&Config::default()).unwrap();
    let mut pinger = client.pinger(addr, PingIdentifier(random())).await;
    pinger.timeout(Duration::from_secs(1));
    tokio::spawn(async move {
        let idx = 0;
        let mut interval = time::interval(Duration::from_secs(1));
        loop {
            let payload = [0; 56];
            let idx = idx + 1;
            interval.tick().await;
            let _ = pinger.ping(PingSequence(idx), &payload).await;
        }
    });
}
