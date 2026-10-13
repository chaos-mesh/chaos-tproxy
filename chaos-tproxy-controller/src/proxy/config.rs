use std::convert::TryFrom;
use std::net::Ipv4Addr;

use anyhow::{anyhow, Error};
use pnet::datalink::NetworkInterface;
use pnet::ipnetwork::IpNetwork;
use chaos_tproxy_proxy::raw_config::{RawConfig as ProxyRawConfig, Role};
use crate::proxy::net::bridge::get_default_interface;

use crate::raw_config::{RawConfig, RawRole};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Config {
    pub proxy_config: ProxyRawConfig,
}

impl TryFrom<RawConfig> for Config {
    type Error = Error;

    fn try_from(raw: RawConfig) -> Result<Self, Self::Error> {
        Self::with_interface(raw, get_default_interface)
    }
}

impl Config {
    fn with_interface(
        raw: RawConfig,
        find_interface: impl FnOnce() -> Result<NetworkInterface, Error>,
    ) -> Result<Self, Error> {
        let ipv4s: Vec<Ipv4Addr> = find_interface()?
            .ips
            .iter()
            .filter_map(|ips| match ips {
                IpNetwork::V4(ipv4) => Some(ipv4),
                _ => None,
            })
            .map(|ipv4| ipv4.ip())
            .collect();
        if ipv4s.is_empty() {
            return Err(anyhow!("no default ipv4"));
        }
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
                listen_port: get_free_port(raw.proxy_ports.clone())?,
                rules: raw.rules.map_or(vec![], |rules| rules),
                role: raw.role.and_then(|role| {
                    Option::from(match role {
                        RawRole::Client => Role::Client(ipv4s),
                        RawRole::Server => Role::Server(ipv4s),
                    })
                }),
                tls: raw.tls,
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
            rules: None,
            tls: None,
            role: None,

            interface: None,
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
                    rules: vec![],
                    role: None,
                    tls: None
                }
            }
        );

        let config: Config = RawConfig {
            proxy_ports: Some(vec![1025u16, 1026u16]),
            safe_mode: Some(true),
            rules: None,
            tls: None,
            role: None,

            interface: None,
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
                    rules: vec![],
                    role: None,
                    tls: None
                }
            }
        );
    }

    fn interface(ips: &[&str]) -> pnet::datalink::NetworkInterface {
        pnet::datalink::NetworkInterface {
            name: "eth0".into(),
            description: String::new(),
            index: 1,
            mac: None,
            ips: ips.iter().map(|ip| ip.parse().unwrap()).collect(),
            flags: libc::IFF_UP as u32,
        }
    }

    #[test]
    fn config_without_role_accepts_ipv6_only_interface() {
        let raw = serde_json::from_str(r#"{"proxy_ports":[8080]}"#).unwrap();
        let config = Config::with_interface(raw, || Ok(interface(&["fe80::1/64"]))).unwrap();
        assert_eq!(config.proxy_config, ProxyRawConfig {
            proxy_ports: Some("8080".into()),
            listen_port: 1025,
            safe_mode: false,
            rules: vec![],
            role: None,
            tls: None,
        });
    }

    #[test]
    fn config_without_role_accepts_missing_interface() {
        let raw = serde_json::from_str(r#"{"proxy_ports":[8080]}"#).unwrap();
        let config = Config::with_interface(raw, || Err(anyhow::anyhow!("no interfaces"))).unwrap();
        assert_eq!(config.proxy_config.proxy_ports, Some("8080".into()));
        assert_eq!(config.proxy_config.listen_port, 1025);
        assert_eq!(config.proxy_config.role, None);
    }

    #[test]
    fn stop_config_does_not_require_role_addresses() {
        for role in ["Client", "Server"] {
            let raw = serde_json::from_value(serde_json::json!({"role": role})).unwrap();
            let config = Config::with_interface(raw, || Ok(interface(&["fe80::1/64"]))).unwrap();
            assert_eq!(config.proxy_config.proxy_ports, None);
            assert_eq!(config.proxy_config.listen_port, 1025);
            assert_eq!(config.proxy_config.role, None);
        }
    }

    #[test]
    fn active_roles_resolve_ipv4_addresses() {
        use chaos_tproxy_proxy::raw_config::Role;
        let ip = "192.0.2.4".parse().unwrap();
        for (role, expected) in [("Client", Role::Client(vec![ip])), ("Server", Role::Server(vec![ip]))] {
            let raw = serde_json::from_value(serde_json::json!({"proxy_ports": [8080], "role": role})).unwrap();
            let config = Config::with_interface(raw, || Ok(interface(&["fe80::1/64", "192.0.2.4/24"]))).unwrap();
            assert_eq!(config.proxy_config.role, Some(expected));
            assert_eq!(config.proxy_config.proxy_ports, Some("8080".into()));
        }
    }

    #[test]
    fn active_role_rejects_missing_ipv4() {
        let raw = serde_json::from_str(r#"{"proxy_ports":[8080],"role":"Client"}"#).unwrap();
        let err = Config::with_interface(raw, || Ok(interface(&["fe80::1/64"]))).unwrap_err();
        assert_eq!(err.to_string(), "no default ipv4");
    }

}
