use std::os::unix::io::AsRawFd;
use std::{io, mem};

use tokio::net::TcpSocket;
use tracing::trace;

pub fn set_ip_transparent(socket: &TcpSocket) -> io::Result<()> {
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

pub fn set_mark(socket: &TcpSocket, mark: i32) -> io::Result<()> {
    unsafe {
        let socket_fd = socket.as_raw_fd();
        let value: libc::c_int = mark;
        let ret = libc::setsockopt(
            socket_fd,
            libc::SOL_SOCKET,
            libc::SO_MARK,
            &value as *const _ as *const _,
            mem::size_of_val(&value) as libc::socklen_t,
        );

        if ret != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    trace!("set mark({})", mark);
    Ok(())
}
