use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

use crate::raw_config::RawConfig;

const MAX_REPLY_BYTES: usize = 64 * 1024;

#[derive(Serialize, Deserialize)]
enum StartupReply {
    Ready,
    Failed(String),
}

#[derive(Debug)]
pub struct BootstrapServer {
    path: PathBuf,
    listener: UnixListener,
    config: Vec<u8>,
}

impl BootstrapServer {
    pub fn bind(path: PathBuf, config: &RawConfig) -> Result<Self> {
        let config = serde_json::to_vec(config)?;
        let listener = UnixListener::bind(&path).context("bind proxy bootstrap socket")?;
        Ok(Self {
            path,
            listener,
            config,
        })
    }

    pub async fn wait_ready(&self, timeout: Duration) -> Result<()> {
        tokio::time::timeout(timeout, async {
            let (mut stream, _) = self.listener.accept().await?;
            stream.write_all(&self.config).await?;
            stream.shutdown().await?;
            let mut reply = Vec::new();
            stream
                .take(MAX_REPLY_BYTES as u64 + 1)
                .read_to_end(&mut reply)
                .await?;
            if reply.is_empty() {
                bail!("proxy closed bootstrap socket before reporting readiness");
            }
            if reply.len() > MAX_REPLY_BYTES {
                bail!("proxy startup reply exceeds size limit");
            }
            match serde_json::from_slice(&reply).context("invalid proxy startup reply")? {
                StartupReply::Ready => Ok(()),
                StartupReply::Failed(error) => Err(anyhow!("proxy startup failed: {}", error)),
            }
        })
        .await
        .context("timed out waiting for proxy readiness")?
    }

    pub fn clear(&self) -> Result<()> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).context("remove proxy bootstrap socket"),
        }
    }
}

pub(crate) struct StartupCompletion(UnixStream);

pub(crate) async fn receive(path: &Path) -> Result<(RawConfig, StartupCompletion)> {
    let mut stream = UnixStream::connect(path).await?;
    let mut config = Vec::new();
    stream.read_to_end(&mut config).await?;
    Ok((serde_json::from_slice(&config)?, StartupCompletion(stream)))
}

impl StartupCompletion {
    pub(crate) async fn ready(self) -> Result<()> {
        self.send(StartupReply::Ready).await
    }

    pub(crate) async fn failed(self, error: &anyhow::Error) -> Result<()> {
        self.send(StartupReply::Failed(
            format!("{error:#}").chars().take(4096).collect(),
        ))
        .await
    }

    async fn send(mut self, reply: StartupReply) -> Result<()> {
        self.0.write_all(&serde_json::to_vec(&reply)?).await?;
        self.0.shutdown().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::oneshot;

    use super::*;

    #[tokio::test]
    async fn readiness_waits_for_child_acknowledgement() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bootstrap.sock");
        let config = RawConfig {
            listen_port: 8080,
            ..Default::default()
        };
        let server = BootstrapServer::bind(path.clone(), &config).unwrap();
        let (received_tx, received_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = oneshot::channel();
        let client = tokio::spawn(async move {
            let (received, completion) = receive(&path).await.unwrap();
            received_tx.send(received).unwrap();
            ready_rx.await.unwrap();
            completion.ready().await.unwrap();
        });
        let ready = server.wait_ready(Duration::from_secs(1));
        tokio::pin!(ready);
        tokio::select! {
            result = &mut ready => panic!("returned before acknowledgement: {:?}", result),
            received = received_rx => assert_eq!(received.unwrap(), config),
        }
        ready_tx.send(()).unwrap();
        ready.await.unwrap();
        client.await.unwrap();
        server.clear().unwrap();
        assert!(!dir.path().join("bootstrap.sock").exists());
    }

    #[tokio::test]
    async fn child_failure_reaches_controller() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bootstrap.sock");
        let server = BootstrapServer::bind(path.clone(), &RawConfig::default()).unwrap();
        let client = tokio::spawn(async move {
            let (_, completion) = receive(&path).await.unwrap();
            completion.failed(&anyhow!("invalid method")).await.unwrap();
        });
        let error = server.wait_ready(Duration::from_secs(1)).await.unwrap_err();
        assert_eq!(error.to_string(), "proxy startup failed: invalid method");
        client.await.unwrap();
    }

    #[tokio::test]
    async fn closed_and_malformed_replies_are_failures() {
        for reply in [b"".as_slice(), b"not json".as_slice()] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("bootstrap.sock");
            let server = BootstrapServer::bind(path.clone(), &RawConfig::default()).unwrap();
            let client = tokio::spawn(async move {
                let (_, mut completion) = receive(&path).await.unwrap();
                completion.0.write_all(reply).await.unwrap();
            });
            let error = server.wait_ready(Duration::from_secs(1)).await.unwrap_err();
            assert_eq!(
                error.to_string(),
                if reply.is_empty() {
                    "proxy closed bootstrap socket before reporting readiness"
                } else {
                    "invalid proxy startup reply"
                }
            );
            client.await.unwrap();
        }
    }

    #[tokio::test]
    async fn missing_child_is_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let server =
            BootstrapServer::bind(dir.path().join("bootstrap.sock"), &RawConfig::default())
                .unwrap();
        let error = server
            .wait_ready(Duration::from_millis(10))
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), "timed out waiting for proxy readiness");
    }
}
