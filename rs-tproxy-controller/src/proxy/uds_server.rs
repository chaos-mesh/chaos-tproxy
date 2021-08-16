use std::io;
use std::path::PathBuf;

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
        tracing::debug!("Uds listener try bind {:?}.", &self.path);
        let listener = UnixListener::bind(self.path.clone())?;
        Ok(listener)
    }

    pub async fn listen(&self, listener: UnixListener) -> anyhow::Result<()> {
        tracing::debug!("Uds listener listening on {:?}.", &self.path);
        loop {
            match (&listener).accept().await {
                Ok((stream, addr)) => {
                    let buf = bincode::serialize(&self.data)?;
                    loop {
                        stream.writable().await?;
                        match stream.try_write(buf.as_slice()) {
                            Ok(_) => {
                                tracing::debug!("Config successfully transferred.");
                                return Ok(());
                            }
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                continue;
                            }
                            Err(e) => {
                                tracing::debug!(
                                    "error : try_write raw config to {:?} failed",
                                    addr
                                );
                                return Err(anyhow::anyhow!("{}", e));
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!("error : accept connection failed");
                    return Err(anyhow::anyhow!("{}", e));
                }
            }
        }
    }
}
