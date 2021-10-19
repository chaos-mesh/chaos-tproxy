use std::env;

use rs_tproxy_controller::proxy::controller::send_config;
use rs_tproxy_proxy::proxy_main;
use rs_tproxy_proxy::raw_config::RawConfig;
use tokio::time::Duration;
use uuid::Uuid;

#[tokio::test]
async fn test_controller() -> anyhow::Result<()> {
    let uds_path = env::temp_dir()
        .join(Uuid::new_v4().to_string())
        .with_extension("sock");
    let data = RawConfig::default();
    tokio::spawn(proxy_main(uds_path.clone()));
    tokio::time::sleep(Duration::from_secs(1)).await;
    send_config(uds_path, &data).await
}
