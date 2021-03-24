use super::tproxy_in::TProxyInSteam;
use super::unix::socketopt;
use std::io;
use tokio::net::{TcpSocket, TcpStream};
pub struct TProxyOutSteam {
    pub inner: TcpStream,
}

impl TProxyOutSteam {
    pub async fn connect(stream_in: &TProxyInSteam, mark: i32) -> io::Result<TProxyOutSteam> {
        let socket = TcpSocket::new_v4()?;
        socketopt::set_ip_transparent(&socket)?;
        socketopt::set_mark(&socket, mark)?;
        // match socket.bind(stream_in.unwrap_ref().peer_addr()?) {
        //     Err(e) => {
        //         if e.kind() == io::ErrorKind::AddrInUse{
        //             println!("connect by local address");
        //         } else {
        //             panic!(e);
        //         }
        //     },
        //     _ => {
        //         println!("connect by peer address")
        //     },
        // };
        socket.set_reuseaddr(true)?;
        let stream = socket.connect(stream_in.unwrap_ref().local_addr()?).await?;
        Ok(TProxyOutSteam { inner: stream })
    }

    pub fn unwrap(self) -> TcpStream {
        self.inner
    }
    pub fn unwrap_ref(&self) -> &TcpStream {
        &self.inner
    }
}

#[tokio::test]
async fn test() -> io::Result<()> {
    let socket = TcpSocket::new_v4()?;
    socketopt::set_ip_transparent(&socket)?;
    socketopt::set_mark(&socket, 255)?;
    let addr = "172.16.5.219:44000".parse().unwrap();
    socket.bind(addr)?;
    let laddr = "172.16.4.158:30001".parse().unwrap();
    let _stream = socket.connect(laddr).await?;
    println!("ok");
    Ok(())
}
