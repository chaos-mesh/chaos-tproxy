use std::env;

use chaos_tproxy_controller_lib::proxy::uds_server::UdsDataServer;
use chaos_tproxy_proxy::uds_client::UdsDataClient;
use tokio::time::Duration;
use uuid::Uuid;

#[tokio::test]
async fn test_uds() {
    let uds_path = env::temp_dir()
        .join(Uuid::new_v4().to_string())
        .with_extension("sock");
    let data = Uuid::new_v4().to_string();

    let uds_server = UdsDataServer::new(data.clone(), uds_path.clone());
    let listener = uds_server.bind().unwrap();
    let server = uds_server.clone();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::new(5, 0)).await;
        let _ = server
            .listen(listener)
            .await
            .map_err(|e| tracing::error!("{:?}", e));
    });

    let client = UdsDataClient::new(uds_path.clone());
    let mut buf: Vec<u8> = vec![];
    let data_o: String = client.read_into(&mut buf).await.unwrap();
    assert_eq!(data, data_o);
}
