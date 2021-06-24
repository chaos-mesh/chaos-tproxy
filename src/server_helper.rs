use std::future::Future;
use std::pin::Pin;

use async_trait::async_trait;
use futures::TryFutureExt;
use tokio::sync::oneshot::{channel, Receiver, Sender};
use tokio::task::{spawn, JoinHandle};
use std::task::{Context, Poll};

#[async_trait]
pub trait SuperServer {
    async fn start(&mut self) -> anyhow::Result<()>;
    async fn stop(&mut self) -> anyhow::Result<()>;
}

#[derive(Debug)]
pub struct ServeHandler {
    sender: Sender<()>,
    handler: JoinHandle<anyhow::Result<()>>,
}

impl ServeHandler {
    pub async fn serve<F, R, E>(with_signal: F) -> Self
    where
        F: FnOnce(Receiver<()>) -> R,
        R: 'static + Send + Future<Output = Result<(), E>>,
        E: 'static + Send + Into<anyhow::Error>,
    {
        let (sender, rx) = channel();
        let handler = spawn(with_signal(rx).map_err(Into::into));
        Self { sender, handler }
    }

    pub async fn stop(self) -> anyhow::Result<()> {
        let ServeHandler { sender, handler } = self;
        let _ = sender.send(());
        let _ = handler.await??;
        Ok(())
    }
}

pub type BoxedSendFuture<T, E> = Pin<Box<dyn 'static + Send + Future<Output = Result<T, E>>>>;
