use super::tproxy_in::{TProxyInListener, TProxyInSteam};
use super::tproxy_out::TProxyOutSteam;
use crate::handler::http::{Config as HandlerConfig, Handler};
use ::std::io;
use crossbeam::channel::unbounded;
use serde_derive::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

#[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
pub struct Config {
    pub port: u16,
    pub mark: i32,
    pub handler_config: HandlerConfig,
}

pub async fn Tproxy(config: Config) -> io::Result<()> {
    let c = &config;
    let listener = TProxyInListener::new((&config).port.clone(), (&config).mark.clone())?;
    loop {
        let stream_in = listener.accept().await?;
        println!(
            "{} -> {}",
            stream_in.unwrap_ref().peer_addr()?,
            stream_in.unwrap_ref().local_addr()?
        );

        let stream_out = TProxyOutSteam::connect(&stream_in, c.mark.clone()).await?;
        println!(
            "{} -> {}",
            stream_out.unwrap_ref().local_addr()?,
            stream_out.unwrap_ref().peer_addr()?
        );

        let (mut stream_in_read, mut stream_in_write) = stream_in.unwrap().into_split();
        let (mut stream_out_read, mut stream_out_write) = stream_out.unwrap().into_split();
        let (in_sender, in_recever) = unbounded();
        let (out_sender, out_recever) = unbounded();

        let c_in = c.clone();
        tokio::spawn(async move {
            let handler = Handler::new(c_in.handler_config, in_sender, out_recever);
            handler
                .handle_stream(stream_in_read, stream_out_write)
                .await;
        });

        let c_out = c.clone();
        tokio::spawn(async move {
            let handler = Handler::new(c_out.handler_config, out_sender, in_recever);
            handler
                .handle_stream(stream_out_read, stream_in_write)
                .await;
        });
    }
}
