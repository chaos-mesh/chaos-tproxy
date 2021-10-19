use std::future::Future;

use anyhow::Result;
use futures::{select, FutureExt};
use tokio::sync::oneshot::{channel, Sender};
use tokio::task::JoinHandle;

#[derive(Debug)]
pub struct Task<T> {
    handler: JoinHandle<Result<Option<T>>>,
    sender: Sender<()>,
}

impl<T> Task<T>
where
    T: 'static + Send,
{
    pub fn start<F>(f: F) -> Self
    where
        F: 'static + Send + Future<Output = Result<T>>,
    {
        let (tx, rx) = channel();
        Self {
            sender: tx,
            handler: tokio::spawn(async move {
                select! {
                    _ = rx.fuse() => Ok(None),
                    ret = f.fuse() => match ret {
                        Ok(v) => Ok(Some(v)),
                        Err(e) => Err(e)
                    },
                }
            }),
        }
    }

    pub async fn stop(self) -> Result<Option<T>> {
        let _ = self.sender.send(());
        self.handler.await?
    }
}
