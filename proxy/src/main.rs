pub mod config;
pub mod generator;
pub mod handler;
pub mod tproxy;
use handler::http::{Action, Config as HandlerConfig, PacketTarget, Selector};
use tproxy::tproxy::{Config as TproxyConfig, Tproxy};

use tokio;

fn main() {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            // let c = TproxyConfig {
            //     port : 58080,
            //     mark:255,
            //     handler_config : HandlerConfig {
            //         packet: PacketTarget::Request,
            //         selector: Selector {
            //             path: Some(b"/rs-tproxy".to_vec()),
            //             method: None,
            //             code: None,
            //             header_fields: None
            //         },
            //         action: Action::Delay(tokio::time::Duration::from_millis(2000)),
            //     },
            // };
            // let c = TproxyConfig {
            //     port : 58080,
            //     mark:255,
            //     handler_config : HandlerConfig {
            //         packet: PacketTarget::Response,
            //         selector: Selector {
            //             path: Some(b"/rs-tproxy".to_vec()),
            //             method: None,
            //             code: None,
            //             header_fields: None
            //         },
            //         action: Action::Replace(b"HTTP/1.1 404\r\n\r\n".to_vec()),
            //     },
            // };
            let c = TproxyConfig {
                port: 58080,
                mark: 255,
                handler_config: HandlerConfig {
                    packet: PacketTarget::Response,
                    selector: Selector {
                        path: Some(b"/rs-tproxy".to_vec()),
                        method: None,
                        code: None,
                        header_fields: None,
                    },
                    action: Action::Abort,
                },
            };
            match Tproxy(c).await {
                Ok(()) => println!("OK!"),
                Err(e) => eprintln!("{}", e),
            };
        })
}
