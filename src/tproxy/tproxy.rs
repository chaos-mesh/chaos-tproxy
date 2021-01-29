use::std::io;
use tokio::io::{AsyncWriteExt,AsyncReadExt};
use super::tproxy_in::{TProxyInListener,TProxyInSteam};
use super::tproxy_out::TProxyOutSteam;

pub async fn Tproxy() -> io::Result<()> {
    let listener = TProxyInListener::new(58080,255)?;
    loop {
        let stream_in = listener.accept().await?;
        println!("{} -> {}",stream_in.unwrap_ref().peer_addr()?,stream_in.unwrap_ref().local_addr()?);
        
        let stream_out = TProxyOutSteam::connect(&stream_in, 255).await?;
        println!("{} -> {}",stream_out.unwrap_ref().local_addr()?,stream_out.unwrap_ref().peer_addr()?);

        let (mut stream_in_read, mut stream_in_write) = stream_in.unwrap().into_split();
        let (mut stream_out_read, mut stream_out_write) = stream_out.unwrap().into_split();

        tokio::spawn(async move {
            let mut buf_in = [0; 1024*2];
            loop {

                let n = match stream_in_read.read(&mut buf_in).await {
                    // socket closed
                    Ok(n) if n == 0 => return,
                    Ok(n) => n,
                    Err(e) => {
                        eprintln!("failed to read from socket; err = {:?}", e);
                        return;
                    }
                };

                //println!("{}",str::from_utf8(&buf_in).unwrap());

                if let Err(e) = stream_out_write.write_all(&buf_in[0..n]).await {
                    eprintln!("failed to write to socket; err = {:?}", e);
                    return;
                }

            }
        });

        tokio::spawn(async move {
            let mut buf_out = [0; 1024*2];
            loop {

                let n = match stream_out_read.read(&mut buf_out).await {
                    // socket closed
                    Ok(n) if n == 0 => return,
                    Ok(n) => n,
                    Err(e) => {
                        eprintln!("failed to read from socket; err = {:?}", e);
                        return;
                    }
                };
                //println!("{}",str::from_utf8(&buf_out).unwrap());
                if let Err(e) = stream_in_write.write_all(&buf_out[0..n]).await {
                    eprintln!("failed to write to socket; err = {:?}", e);
                    return;
                }
            }
        });
    }
}