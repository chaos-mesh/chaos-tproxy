use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf, Stdin, Stdout};
use tokio::sync::oneshot;

#[derive(Debug)]
pub struct StdStream {
    stdin: Stdin,
    stdout: Stdout,
    eof_sender: Option<oneshot::Sender<()>>,
}

impl StdStream {
    pub(super) fn new() -> (Self, oneshot::Receiver<()>) {
        let (eof_sender, eof_receiver) = oneshot::channel();
        (
            Self {
                stdin: tokio::io::stdin(),
                stdout: tokio::io::stdout(),
                eof_sender: Some(eof_sender),
            },
            eof_receiver,
        )
    }
}

impl AsyncRead for StdStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let remaining = buf.remaining();
        let filled = buf.filled().len();
        let result = Pin::new(&mut self.stdin).poll_read(cx, buf);
        if matches!(result, Poll::Ready(Ok(()))) && remaining > 0 && buf.filled().len() == filled {
            if let Some(sender) = self.eof_sender.take() {
                let _ = sender.send(());
            }
        }
        result
    }
}

impl AsyncWrite for StdStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stdout).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.stdout).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.stdout).poll_shutdown(cx)
    }
}
