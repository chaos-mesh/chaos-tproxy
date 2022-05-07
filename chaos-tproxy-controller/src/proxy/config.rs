use std::convert::TryFrom;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::mpsc::channel;

use anyhow::{anyhow, Error, Result};
use chaos_tproxy_proxy::raw_config::RawConfig as ProxyRawConfig;
use trust_dns_resolver::system_conf::read_system_conf;
use trust_dns_resolver::Resolver;

use crate::raw_config::RawConfig;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Config {
    pub proxy_config: ProxyRawConfig,
}

impl TryFrom<RawConfig> for Config {
    type Error = Error;

    fn try_from(raw: RawConfig) -> Result<Self, Self::Error> {
        Ok(Config {
            proxy_config: ProxyRawConfig {
                proxy_ports: raw.proxy_ports.clone().map(|c| {
                    c.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                }),
                proxy_ips: raw.proxy_domains.map(|domains| {
                    let ips: Vec<Ipv4Addr> = domains
                        .into_iter()
                        .map(|domain| {
                            let (config, opt) = read_system_conf()?;
                            let resolver = Resolver::new(config, opt)?;

                            let (sender, receiver) = channel();
                            let _ = std::thread::spawn(move || {
                                let _ = sender.send(resolver.lookup_ip(domain));
                            })
                            .join();

                            let rsp = receiver.recv()??;
                            let ips: Vec<Ipv4Addr> = rsp
                                .iter()
                                .filter_map(|ip| match ip {
                                    IpAddr::V4(ipv4) => Some(ipv4),
                                    IpAddr::V6(_) => None,
                                })
                                .collect();
                            Ok(ips)
                        })
                        .filter_map(|r: Result<Vec<Ipv4Addr>>| {
                            r.map_err(|e| tracing::error!("resolve domain with error: {}", e))
                                .ok()
                        })
                        .flatten()
                        .collect();
                    ips
                }),
                safe_mode: match &raw.safe_mode {
                    Some(b) => *b,
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

pub(crate) fn get_free_port(ports: Option<Vec<u16>>) -> anyhow::Result<u16> {
    for port in 1025..u16::MAX {
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
