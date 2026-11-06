use std::env;
use std::time::Duration;

use chaos_tproxy_proxy::bootstrap::BootstrapServer;
use chaos_tproxy_proxy::proxy_main;
use chaos_tproxy_proxy::raw_config::{RawConfig, RawFile, TLSRawConfig};
use uuid::Uuid;

#[tokio::test]
async fn test_uds_reports_child_config_failure() {
    let path = env::temp_dir()
        .join(Uuid::new_v4().to_string())
        .with_extension("sock");
    let config = RawConfig {
        tls: Some(TLSRawConfig {
            cert_file: RawFile::Path(path.with_extension("missing")),
            ..Default::default()
        }),
        ..Default::default()
    };
    let server = BootstrapServer::bind(path.clone(), &config).unwrap();
    let child = tokio::spawn(proxy_main(path.clone()));
    let error = server.wait_ready(Duration::from_secs(2)).await.unwrap_err();
    assert!(
        error.to_string().contains("No such file or directory"),
        "{:#}",
        error
    );
    assert!(child.await.unwrap().is_err());
    server.clear().unwrap();
    assert!(!path.exists());
}
