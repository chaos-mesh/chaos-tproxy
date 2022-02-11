use std::convert::TryFrom;

use anyhow::{anyhow, Error};
use chaos_tproxy_proxy::raw_config::RawConfig as ProxyRawConfig;

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

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use chaos_tproxy_proxy::raw_config::RawConfig as ProxyRawConfig;

    use crate::proxy::config::{get_free_port, Config};
    use crate::raw_config::RawConfig;

    #[test]
    fn test_get_free_port() {
        assert!(get_free_port(Some((u16::MIN..u16::MAX).collect())).is_err());
    }

    #[test]
    fn test_try_into() {
        let config: Config = RawConfig {
            proxy_ports: None,
            safe_mode: None,
            interface: None,
            rules: None,

            listen_port: None,
            proxy_mark: None,
            ignore_mark: None,
            route_table: None,
        }
        .try_into()
        .unwrap();
        assert_eq!(
            config,
            Config {
                proxy_config: ProxyRawConfig {
                    proxy_ports: None,
                    listen_port: get_free_port(None).unwrap(),
                    safe_mode: false,
                    interface: None,
                    rules: vec![]
                }
            }
        );

        let config: Config = RawConfig {
            proxy_ports: Some(vec![1025u16, 1026u16]),
            safe_mode: Some(true),
            interface: Some("ens33".parse().unwrap()),
            rules: None,

            listen_port: None,
            proxy_mark: None,
            ignore_mark: None,
            route_table: None,
        }
        .try_into()
        .unwrap();
        assert_eq!(
            config,
            Config {
                proxy_config: ProxyRawConfig {
                    proxy_ports: Some("1025,1026".parse().unwrap()),
                    listen_port: 1027u16,
                    safe_mode: true,
                    interface: Some("ens33".parse().unwrap()),
                    rules: vec![]
                }
            }
        );
    }
}
