use std::path::PathBuf;

use tokio::net::UnixListener;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone)]
pub struct UdsDataServer<T> {
    pub data: T,
    pub path: PathBuf,
}

impl<T: serde::ser::Serialize> UdsDataServer<T> {
    pub fn new(data: T, path: PathBuf) -> Self {
        Self { data, path }
    }

    pub fn bind(&self) -> anyhow::Result<UnixListener> {
        tracing::debug!("Uds listener try bind {:?}.", &self.path);
        let listener = UnixListener::bind(self.path.clone())?;
        Ok(listener)
    }

    pub fn clear(&self) -> anyhow::Result<()> {
        std::fs::remove_file(&self.path)?;
        Ok(())
    }

    pub async fn listen(&self, listener: UnixListener) -> anyhow::Result<()> {
        tracing::debug!("Uds listener listening on {:?}.", &self.path);
        loop {
            match (&listener).accept().await {
                Ok((mut stream, addr)) => {
                    let buf = bincode::serialize(&self.data)?;
                    tokio::spawn(async move {
                        return match stream.write_all(buf.as_slice()).await {
                            Ok(_) => {
                                tracing::debug!("Config successfully transferred.");
                                Ok(())
                            }
                            Err(e) => {
                                tracing::info!(
                                    "error : write_all raw config to {:?} failed",
                                    addr
                                );
                                Err(anyhow::anyhow!("{}", e))
                            }
                        }
                    });
                }
                Err(e) => {
                    tracing::info!("error : accept connection failed");
                    return Err(anyhow::anyhow!("{}", e));
                }
            }
        }
    }
}
