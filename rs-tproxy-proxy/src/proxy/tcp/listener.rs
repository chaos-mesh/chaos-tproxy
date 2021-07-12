use std::io;
use std::net::SocketAddr;

use tokio::net::{self, TcpStream};
use tracing::{debug, instrument, trace};

use crate::proxy::tcp::transparent_socket::TransparentSocket;

/// A stream of connections from binding to an address.
/// As an implementation of `hyper::server::accept::Accept`.
#[must_use = "streams do nothing unless polled"]
pub struct TcpListener {
    listener: net::TcpListener,
    tcp_nodelay: bool,
}

impl TcpListener {
    /// Creates a new `TcpIncoming` binding to provided socket address.
    #[instrument]
    pub fn bind(addr: SocketAddr) -> io::Result<Self> {
        let socket = TransparentSocket::bind(addr)?;

        Ok(Self {
            listener: socket.listen(1024)?,
            tcp_nodelay: true,
        })
    }

    /// Set the value of `TCP_NODELAY` option for accepted connections.
    pub fn set_nodelay(&mut self, enabled: bool) -> &mut Self {
        self.tcp_nodelay = enabled;
        self
    }

    /// accept TcpStream.
    pub async fn accept(&self) -> io::Result<TcpStream> {
        loop {
            match self.listener.accept().await {
                Ok((stream, _)) => {
                    if let Err(e) = stream.set_nodelay(self.tcp_nodelay) {
                        trace!("error trying to set TCP nodelay: {}", e);
                    }
                    return Ok(stream);
                }
                Err(e) => {
                    // Connection errors can be ignored directly, continue by
                    // accepting the next request.
                    if is_connection_error(&e) {
                        debug!("accepted connection already errored: {}", e);
                        continue;
                    } else {
                        return Err(e);
                    }
                }
            };
        }
    }
}

/// This function defines errors that are per-connection. Which basically
/// means that if we get this error from `accept()` system call it means
/// next connection might be ready to be accepted.
///
/// All other errors will incur a timeout before next `accept()` is performed.
/// The timeout is useful to handle resource exhaustion errors like ENFILE
/// and EMFILE. Otherwise, could enter into tight loop.
pub fn is_connection_error(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
    )
}
