use std::path::PathBuf;

use tokio::io::AsyncReadExt;
use tokio::net::UnixStream;

/// UdsDataClient is designed **ONLY** for communicate between chaos-tproxy main process and chaos-tproxy child process.
/// It's not a general purpose Unix Domain Socket client.
///
/// UdsDataClient would create a connection to a certain Unix Domain Socket, and read the serialized data from that
/// socket, then try to deserialize data to the required type.
#[derive(Debug, Clone)]
pub struct UdsDataClient {
    pub path: PathBuf,
}

impl UdsDataClient {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// read_into would create a new connection to the target Unix Domain Socket and receive the serialized data from
    /// server-side, then try to deserialize data to the required type.
    ///
    /// It would establish a new connection every time, so it's safe to call this method multiple times.
    pub async fn read_into<'a, T: serde::de::Deserialize<'a>>(
        &self,
        buf: &'a mut Vec<u8>,
    ) -> anyhow::Result<T> {
        tracing::debug!("try connect path : {:?}", &self.path);
        let mut stream = UnixStream::connect(self.path.clone()).await?;
        return match stream.read_to_end(buf).await {
            Ok(_) => {
                tracing::debug!("Read data successfully.");

                match serde_json::from_slice(buf.as_slice()) {
                    Ok(o) => {
                        tracing::debug!("Deserialize data successfully.");
                        Ok(o)
                    }
                    Err(e) => {
                        tracing::debug!("Deserialize data failed.");
                        Err(anyhow::anyhow!("{}", e))
                    }
                }
            }
            Err(e) => {
                tracing::debug!("Read data failed with err {:?}.", e);
                Err(anyhow::anyhow!("{}", e))
            }
        };
    }
}
