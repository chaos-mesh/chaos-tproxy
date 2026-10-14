use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::{Bytes, BytesMut};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf, Stdin, Stdout};

#[derive(Debug)]
pub struct StdStream {
    stdin: Stdin,
    stdout: Stdout,
    pending: Bytes,
    eof: bool,
}

impl StdStream {
    pub(super) fn new() -> Self {
        Self {
            stdin: tokio::io::stdin(),
            stdout: tokio::io::stdout(),
            pending: Bytes::new(),
            eof: false,
        }
    }

    pub(super) fn prepend(&mut self, prefix: Bytes) {
        if self.pending.is_empty() {
            self.pending = prefix;
        } else if !prefix.is_empty() {
            let mut pending = BytesMut::with_capacity(prefix.len() + self.pending.len());
            pending.extend_from_slice(&prefix);
            pending.extend_from_slice(&self.pending);
            self.pending = pending.freeze();
        }
    }

    pub(super) fn is_exhausted(&self) -> bool {
        self.eof && self.pending.is_empty()
    }
}

impl AsyncRead for StdStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let remaining = buf.remaining();
        if remaining == 0 {
            return Poll::Ready(Ok(()));
        }
        if !self.pending.is_empty() {
            let len = remaining.min(self.pending.len());
            buf.put_slice(&self.pending.split_to(len));
            return Poll::Ready(Ok(()));
        }
        if self.eof {
            return Poll::Ready(Ok(()));
        }
        let filled = buf.filled().len();
        let result = Pin::new(&mut self.stdin).poll_read(cx, buf);
        if matches!(result, Poll::Ready(Ok(()))) && buf.filled().len() == filled {
            self.eof = true;
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
