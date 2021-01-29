use std::{io, net::{IpAddr, Ipv4Addr, SocketAddr}};
use tokio::net::{TcpListener, TcpSocket, TcpStream};
use super::unix::socketopt;

pub struct TProxyInListener{
    inner : TcpListener,
}

impl TProxyInListener {
    pub fn new(port:u16,mark:i32) -> io::Result<TProxyInListener>{
        let socket = TcpSocket::new_v4()?;
        socketopt::set_ip_transparent(&socket)?;
        socket.set_reuseaddr(true)?;
        socket.bind(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            port,
        ))?;
        match socketopt::set_mark(&socket, mark) {
            Err(e) => {
                if e.kind() == io::ErrorKind::Other {
                    println!("{}",e);
                } else {
                    panic!(e);
                }
            },
            _ => {
                println!("output with mark 255");
            }
        }
        let l = socket.listen(1024)?;
        Ok(TProxyInListener{
            inner : l,
        })
    }
    
    pub async fn accept(&self) -> io::Result<TProxyInSteam> {
        let (stream_in, _) = self.inner.accept().await?;
        return Ok(TProxyInSteam::new(stream_in));
    }
}

pub struct TProxyInSteam {
    inner : TcpStream
}

impl TProxyInSteam {
    fn new(stream : TcpStream) -> TProxyInSteam {
        TProxyInSteam {
            inner : stream,
        }
    }
    pub fn unwrap(self) -> TcpStream {
        self.inner
    }
    pub fn unwrap_ref(&self) -> &TcpStream {
        &self.inner
    }
}