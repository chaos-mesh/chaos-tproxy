use std::convert::TryFrom;

use anyhow::{anyhow, Error};
use rs_tproxy_proxy::raw_config::RawConfig as ProxyRawConfig;

use crate::raw_config::RawConfig;

#[derive(Debug, Clone)]
pub struct Config {
    pub proxy_config: ProxyRawConfig,
}

impl TryFrom<RawConfig> for Config {
    type Error = Error;

    fn try_from(raw: RawConfig) -> Result<Self, Self::Error> {
        Ok(Config {
            proxy_config: ProxyRawConfig {
                proxy_ports: match raw.proxy_ports.clone() {
                    Some(c) => Some(
                        c.iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(","),
                    ),
                    None => None,
                },
                safe_mode: match &raw.safe_mode {
                    Some(b) => b.clone(),
                    None => false,
                },
                interface: raw.interface,
                listen_port: get_free_port(raw.proxy_ports.clone())?,
                rules: match raw.rules {
                    Some(rules) => rules,
                    None => vec![],
                },
            },
        })
    }
}

fn get_free_port(ports: Option<Vec<u16>>) -> anyhow::Result<u16> {
    for port in 1025..65535 {
        match &ports {
            None => {
                return Ok(port);
            }
            Some(ports) => {
                if ports.iter().all(|&p| p != port) {
                    return Ok(port);
                }
            }
        };
    }
    Err(anyhow!(
        "never apply all ports in 1025-65535 to be proxy ports"
    ))
}
