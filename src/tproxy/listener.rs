use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{self, Poll, Context};
use std::time::Duration;
use std::{fmt, io, matches};

use hyper::server::accept::Accept;
use tokio::net::{TcpListener, TcpSocket, TcpStream};
use tokio::time::{sleep, Sleep};
use tracing::{debug, error, instrument, trace};

use super::socketopt;
/// A stream of connections from binding to an address.
/// As an implementation of `hyper::server::accept::Accept`.
#[must_use = "streams do nothing unless polled"]
pub struct TcpIncoming {
    addr: SocketAddr,
    listener: TcpListener,
    sleep_on_errors: bool,
    tcp_nodelay: bool,
    timeout: Option<Pin<Box<Sleep>>>,
}

impl TcpIncoming {
    /// Creates a new `TcpIncoming` binding to provided socket address.
    #[instrument]
    pub fn bind(addr: SocketAddr, mark: i32) -> io::Result<Self> {
        let socket = TcpSocket::new_v4()?;
        socketopt::set_ip_transparent(&socket)?;
        socket.set_reuseaddr(true)?;
        socket.bind(addr)?;
        socketopt::set_mark(&socket, mark)?;
        TcpIncoming::new(socket.listen(1024)?)
    }

    pub fn new(listener: TcpListener) -> io::Result<Self> {
        let addr = listener.local_addr()?;
        Ok(TcpIncoming {
            listener,
            addr,
            sleep_on_errors: true,
            tcp_nodelay: false,
            timeout: None,
        })
    }

    /// Get the local address bound to this listener.
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Set the value of `TCP_NODELAY` option for accepted connections.
    pub fn set_nodelay(&mut self, enabled: bool) -> &mut Self {
        self.tcp_nodelay = enabled;
        self
    }

    /// Set whether to sleep on accept errors.
    ///
    /// A possible scenario is that the process has hit the max open files
    /// allowed, and so trying to accept a new connection will fail with
    /// `EMFILE`. In some cases, it's preferable to just wait for some time, if
    /// the application will likely close some files (or connections), and try
    /// to accept the connection again. If this option is `true`, the error
    /// will be logged at the `error` level, since it is still a big deal,
    /// and then the listener will sleep for 1 second.
    ///
    /// In other cases, hitting the max open files should be treat similarly
    /// to being out-of-memory, and simply error (and shutdown). Setting
    /// this option to `false` will allow that.
    ///
    /// Default is `true`.
    pub fn set_sleep_on_errors(&mut self, val: bool) {
        self.sleep_on_errors = val;
    }

    /// Poll TcpStream.
    fn poll_next(&mut self, cx: &mut task::Context<'_>) -> Poll<io::Result<TcpStream>> {
        // Check if a previous timeout is active that was set by IO errors.
        if let Some(ref mut to) = self.timeout {
            futures::ready!(Pin::new(to).poll(cx));
        }
        self.timeout = None;

        loop {
            match futures::ready!(self.listener.poll_accept(cx)) {
                Ok((socket, _)) => {
                    if let Err(e) = socket.set_nodelay(self.tcp_nodelay) {
                        trace!("error trying to set TCP nodelay: {}", e);
                    }
                    return Poll::Ready(Ok(socket));
                }
                Err(e) => {
                    // Connection errors can be ignored directly, continue by
                    // accepting the next request.
                    if is_connection_error(&e) {
                        debug!("accepted connection already errored: {}", e);
                        continue;
                    }

                    if self.sleep_on_errors {
                        error!("accept error: {}", e);

                        // Sleep 1s.
                        let mut timeout = Box::pin(sleep(Duration::from_secs(1)));

                        match timeout.as_mut().poll(cx) {
                            Poll::Ready(()) => {
                                // Wow, it's been a second already? Ok then...
                                continue;
                            }
                            Poll::Pending => {
                                self.timeout = Some(timeout);
                                return Poll::Pending;
                            }
                        }
                    } else {
                        return Poll::Ready(Err(e));
                    }
                }
            }
        }
    }
}

impl Accept for TcpIncoming {
    type Conn = TcpStream;
    type Error = io::Error;

    #[inline]
    fn poll_accept(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
        let stream = futures::ready!(self.poll_next(cx))?;
        trace!("accept tcp stream to {}", stream.local_addr().unwrap());
        Poll::Ready(Some(Ok(stream)))
    }
}

impl Future for TcpIncoming {
    type Output = Option<io::Result<TcpStream>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.as_mut().poll_accept(cx)
    }
}

/// This function defines errors that are per-connection. Which basically
/// means that if we get this error from `accept()` system call it means
/// next connection might be ready to be accepted.
///
/// All other errors will incur a timeout before next `accept()` is performed.
/// The timeout is useful to handle resource exhaustion errors like ENFILE
/// and EMFILE. Otherwise, could enter into tight loop.
fn is_connection_error(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
    )
}

impl fmt::Debug for TcpIncoming {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TcpIncoming")
            .field("addr", &self.addr)
            .field("sleep_on_errors", &self.sleep_on_errors)
            .field("tcp_nodelay", &self.tcp_nodelay)
            .finish()
    }
}
