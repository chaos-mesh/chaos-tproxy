use std::fs::File;
use std::io::prelude::*;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
use tokio::io::unix::AsyncFd;
use std::io::Error;
use tokio::io::{AsyncWriteExt,AsyncWrite,AsyncReadExt,AsyncRead};
use tokio::net::TcpStream;
use std::pin::Pin;
use std::io;
use std::task::{Context, Poll};
use futures::ready;
use mio::unix::pipe;
use tokio::net::tcp::{OwnedReadHalf,OwnedWriteHalf};
use nix::fcntl::{SpliceFFlags, splice};
pub struct AsyncPipe {
    reader: AsyncPipeReader,
    writer: AsyncPipeWriter,
}

impl AsyncPipe {
    pub fn new() -> io::Result<AsyncPipe> {
        let (sender, receiver) = pipe::new()?;
        return Ok(AsyncPipe {
            reader : AsyncPipeReader::new(AsyncFd::new(receiver)?),
            writer : AsyncPipeWriter::new(AsyncFd::new(sender)?)
        });
    }
    pub fn split(self) -> (AsyncPipeReader,AsyncPipeWriter) {
        (self.reader,self.writer)
    }
}

pub struct AsyncPipeReader {
    inner: AsyncFd<pipe::Receiver>,
}

impl AsyncPipeReader {
    fn new(reader: AsyncFd<pipe::Receiver>) -> AsyncPipeReader {
        return Self { inner: reader };
    }
    pub async fn read(&self, out: &mut [u8]) -> io::Result<usize> {
        loop {
            let mut guard = self.inner.readable().await?;
            match guard.try_io(|inner| inner.get_ref().read(out)) {
                Ok(result) => return result,
                Err(_would_block) => continue,
            }
        }
    }
}

impl AsRawFd for AsyncPipeReader {
    fn as_raw_fd(&self) -> RawFd {
        return self.inner.as_raw_fd();
    }
}

pub struct AsyncPipeWriter {
    inner: AsyncFd<pipe::Sender>,
}

impl AsyncPipeWriter {
    fn new(writer: AsyncFd<pipe::Sender>) -> AsyncPipeWriter {
        return Self { inner: writer };
    }

    pub async fn write(&self, input: &mut [u8]) -> io::Result<usize> {
        loop {
            let mut guard = self.inner.writable().await?;
            match guard.try_io(|inner| inner.get_ref().write(input)) {
                Ok(result) => return result,
                Err(_would_block) => continue,
            }
        }
    }
}

impl AsRawFd for AsyncPipeWriter {
    fn as_raw_fd(&self) -> RawFd {
        return self.inner.as_raw_fd();
    }
}

impl AsyncWrite for AsyncPipeWriter {
    fn poll_write(
         self: Pin<&mut Self>,
       cx: &mut Context<'_>,
       buf: &[u8]
   ) -> Poll<io::Result<usize>> {
       loop {
           let mut guard = ready!(self.inner.poll_write_ready(cx))?;

           match guard.try_io(|inner| inner.get_ref().write(buf)) {
               Ok(result) => return Poll::Ready(result),
               Err(_would_block) => continue,
           }
       }
   }

   fn poll_flush(
       self: Pin<&mut Self>,
       cx: &mut Context<'_>,
   ) -> Poll<io::Result<()>> {
       Poll::Ready(self.inner.get_ref().flush())
   }

   fn poll_shutdown(
       self: Pin<&mut Self>,
       cx: &mut Context<'_>,
   ) -> Poll<io::Result<()>> {
       self.inner.get_ref().flush()?;
       Poll::Ready(Ok(()))
   }

   fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        let buf = bufs
            .iter()
            .find(|b| !b.is_empty())
            .map_or(&[][..], |b| &**b);
        self.poll_write(cx, buf)
    }

   fn is_write_vectored(&self) -> bool {
        false
    }
}

pub async fn splice_socket_to_pipe_all(rh:&OwnedReadHalf,pw:&AsyncPipeWriter) -> io::Result<()> {
    loop {
        let ret = splice_socket_to_pipe(rh,pw).await?;
        if ret == 0 {
            return Ok(());
        }
        if ret > 0 {
            continue;
        }
    }
}

pub async fn splice_socket_to_pipe(rh:&OwnedReadHalf,pw:&AsyncPipeWriter) -> io::Result<usize> {
    loop {
        let socket_guard = rh.as_ref().readable().await?;
        let mut pipe_guard = pw.inner.writable().await?;
        match pipe_guard.try_io(
            |inner| try_splice_to_pipe(
                rh.as_ref().as_raw_fd(),
                inner.as_raw_fd())
        ) {
            Ok(result) => return result,
            Err(_would_block) => continue,
        }
    }
}

fn try_splice_to_pipe(
    fd_in: RawFd,
    fd_out: RawFd
) -> io::Result<usize> {
    match splice(fd_in,None,fd_out,None,16*1024,SpliceFFlags::SPLICE_F_NONBLOCK|SpliceFFlags::SPLICE_F_MOVE) {
        Ok(result) => return Ok(result),
        Err(e) => return Err(std::io::Error::from(e.as_errno().unwrap()))
    };
}

#[cfg(test)]
mod tests {
    use super::AsyncPipe;
    use std::io;
    use tokio::io::AsyncWriteExt;
    #[tokio::test]
    async fn test_pipe() -> io::Result<()> {
        let (reader,writer) = AsyncPipe::new()?.split();
        
        Ok(())
    }
}