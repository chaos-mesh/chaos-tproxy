use std::net::SocketAddr;
use std::os::unix::io::AsRawFd;
use std::{io, mem};

use tokio::net::{TcpSocket, TcpStream};

/// A socket generator with IP_TRANSPARENT flag.
/// User can Clone this instead of clone a linux socket which may bring mistake.
#[derive(Debug, Clone)]
pub struct TransparentSocket {
    addr: SocketAddr,
}

impl TransparentSocket {
    pub fn new(addr: SocketAddr) -> TransparentSocket {
        Self { addr }
    }

    pub fn bind(addr: SocketAddr) -> io::Result<TcpSocket> {
        let socket = TransparentSocket::set_socket()?;
        socket.bind(addr)?;
        Ok(socket)
    }

    pub async fn conn(&self, dist: SocketAddr) -> io::Result<TcpStream> {
        let socket = TransparentSocket::set_socket()?;
        socket.bind(self.addr)?;
        socket.connect(dist).await
    }

    fn set_socket() -> io::Result<TcpSocket> {
        let socket = TcpSocket::new_v4()?;
        TransparentSocket::set_ip_transparent(&socket)?;
        socket.set_reuseaddr(true)?;
        Ok(socket)
    }

    /// Set IP_TRANSPARENT for use of tproxy.
    /// User may need to get root privilege to use it.
    fn set_ip_transparent(socket: &TcpSocket) -> io::Result<()> {
        unsafe {
            let socket_fd = socket.as_raw_fd();
            let enable: libc::c_int = 1;
            let ret = libc::setsockopt(
                socket_fd,
                libc::SOL_IP,
                libc::IP_TRANSPARENT,
                &enable as *const _ as *const _,
                mem::size_of_val(&enable) as libc::socklen_t,
            );

            if ret != 0 {
                return Err(io::Error::last_os_error());
            }
        };
        Ok(())
    }
}
