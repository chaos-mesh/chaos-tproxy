use std::io;
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
        loop {
            stream.readable().await?;

            match stream.read_to_end(buf).await {
                Ok(_) => {
                    tracing::debug!("Read data successfully.");

                    return match bincode::deserialize(buf.as_slice()) {
                        Ok(o) => {
                            tracing::debug!("Deserialize data successfully.");
                            Ok(o)
                        }
                        Err(e) => {
                            tracing::debug!("Deserialize data failed.");
                            Err(anyhow::anyhow!("{}", e))
                        }
                    };
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    tracing::debug!("Read data failed with err {:?}.", e);
                    return Err(anyhow::anyhow!("{}", e));
                }
            }
        }
    }
}
