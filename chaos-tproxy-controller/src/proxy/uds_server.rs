use std::path::PathBuf;

use tokio::io::AsyncWriteExt;
use tokio::net::UnixListener;

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
        tracing::info!("Uds listener try binding {:?}", &self.path);
        let listener = UnixListener::bind(self.path.clone())?;
        Ok(listener)
    }

    pub fn clear(&self) -> anyhow::Result<()> {
        std::fs::remove_file(&self.path)?;
        Ok(())
    }

    pub async fn listen(&self, listener: UnixListener) -> anyhow::Result<()> {
        tracing::info!("Uds listener listening on {:?}", &self.path);
        loop {
            match (listener).accept().await {
                Ok((mut stream, addr)) => {
                    let buf = serde_json::to_vec(&self.data)?;
                    tokio::spawn(async move {
                        match stream.write_all(buf.as_slice()).await {
                            Ok(_) => {
                                tracing::info!("Uds server Config successfully transferred.");
                                Ok(())
                            }
                            Err(e) => {
                                tracing::error!(
                                    "error : write_all raw config to {:?} failed",
                                    addr
                                );
                                Err(anyhow::anyhow!("{}", e))
                            }
                        }
                    });
                }
                Err(e) => {
                    tracing::error!("error : accept connection failed");
                    return Err(anyhow::anyhow!("{}", e));
                }
            }
        }
    }
}
