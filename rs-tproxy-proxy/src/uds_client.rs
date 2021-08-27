use std::path::PathBuf;

use tokio::io::AsyncReadExt;
use tokio::net::UnixStream;

#[derive(Debug, Clone)]
pub struct UdsDataClient {
    pub path: PathBuf,
}

impl UdsDataClient {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub async fn read_into<'a, T: serde::de::Deserialize<'a>>(
        &self,
        buf: &'a mut Vec<u8>,
    ) -> anyhow::Result<T> {
        tracing::debug!("try connect path : {:?}", &self.path);
        let mut stream = UnixStream::connect(self.path.clone()).await?;
        return match stream.read_to_end(buf).await {
            Ok(_) => {
                tracing::debug!("Read data successfully.");

                match bincode::deserialize(buf.as_slice()) {
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
