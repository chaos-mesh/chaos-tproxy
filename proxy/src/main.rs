#![feature(duration_constants)]

pub mod cmd;
pub mod config;
pub mod generator;
pub mod handler;
pub mod tproxy;
pub mod util;

use tproxy::tproxy::tproxy;

use crate::cmd::proxy::get_config;
use tokio;

fn main() {
    let c = get_config();
    println!("{:?}", c);
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            match tproxy(c.tproxy_config.clone()).await {
                Ok(()) => println!("OK!"),
                Err(e) => eprintln!("{}", e),
            };
        })
}
