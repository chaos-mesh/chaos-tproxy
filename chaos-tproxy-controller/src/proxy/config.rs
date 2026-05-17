use std::convert::TryFrom;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, Error};
use pnet::ipnetwork::IpNetwork;

use chaos_tproxy_proxy::raw_config as proxy;

use crate::proxy::net::bridge::get_default_interface;
use crate::raw_config::{
    PatchAction, RawConfig, RawFile, RawFileContents, RawFilePath, ReplaceAction,
    ReplaceBodyContents, Role, RoleClient, RoleServer, Rule, Target, TLSRawConfig,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Config {
    pub proxy_config: proxy::RawConfig,
}

impl TryFrom<RawConfig> for Config {
    type Error = Error;

    fn try_from(raw: RawConfig) -> Result<Self, Self::Error> {
        let ipv4s: Vec<Ipv4Addr> = get_default_interface()?
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

        let proxy_ports = raw
            .proxy_ports
            .map(|ports| {
                ports
                    .into_iter()
                    .map(|p| p.get() as u16)
                    .collect::<Vec<u16>>()
            });

        Ok(Config {
            proxy_config: proxy::RawConfig {
                proxy_ports: proxy_ports.clone().map(|ports| {
                    ports
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                }),
                safe_mode: raw.safe_mode.unwrap_or(false),
                listen_port: get_free_port(proxy_ports)?,
                rules: raw
                    .rules
                    .unwrap_or_default()
                    .into_iter()
                    .map(convert_rule)
                    .collect::<Result<Vec<_>, _>>()?,
                role: raw.role.map(|role| match role {
                    Role::Client(RoleClient { client }) => proxy::Role::Client(client),
                    Role::Server(RoleServer { server }) => proxy::Role::Server(server),
                }),
                tls: raw.tls.map(convert_tls).transpose()?,
            },
        })
    }
}

fn convert_rule(rule: Rule) -> Result<proxy::RawRule, Error> {
    Ok(proxy::RawRule {
        target: match rule.target {
            Target::Request => proxy::RawTarget::Request,
            Target::Response => proxy::RawTarget::Response,
        },
        selector: proxy::RawSelector {
            port: rule.selector.port.map(|p: std::num::NonZeroU32| p.get() as u16),
            path: rule.selector.path,
            method: rule.selector.method,
            code: rule
                .selector
                .code
                .map(|c| u16::try_from(c).map_err(|_| anyhow!("invalid status code: {c}")))
                .transpose()?,
            request_headers: rule.selector.request_headers,
            response_headers: rule.selector.response_headers,
        },
        actions: proxy::RawActions {
            abort: rule.actions.abort,
            delay: rule
                .actions
                .delay
                .as_deref()
                .map(|s| {
                    humantime::parse_duration(s)
                        .map(Duration::from)
                        .map_err(|e| anyhow!("invalid delay {:?}: {e}", s))
                })
                .transpose()?,
            replace: rule.actions.replace.map(convert_replace).transpose()?,
            patch: rule.actions.patch.map(convert_patch).transpose()?,
        },
    })
}

fn convert_replace(r: ReplaceAction) -> Result<proxy::RawReplaceAction, Error> {
    Ok(proxy::RawReplaceAction {
        path: r.path,
        method: r.method,
        body: r.body.map(|b| proxy::RawReplaceBody {
            contents: match b.contents {
                ReplaceBodyContents::Text(t) => proxy::RawReplaceBodyContents::TEXT(t.value),
                ReplaceBodyContents::Base64(b) => proxy::RawReplaceBodyContents::BASE64(b.value),
            },
        }),
        code: r
            .code
            .map(|c| u16::try_from(c).map_err(|_| anyhow!("invalid status code: {c}")))
            .transpose()?,
        queries: r.queries,
        headers: r.headers,
    })
}

fn convert_patch(p: PatchAction) -> Result<proxy::RawPatchAction, Error> {
    Ok(proxy::RawPatchAction {
        body: p.body.map(|b| proxy::RawPatchBody {
            contents: proxy::RawPatchBodyContents::JSON(b.contents.0.value),
        }),
        queries: p.queries.map(|qs: Vec<[String; 2]>| {
            qs.into_iter().map(|[k, v]| (k, v)).collect()
        }),
        headers: p.headers.map(|hs: Vec<[String; 2]>| {
            hs.into_iter().map(|[k, v]| (k, v)).collect()
        }),
    })
}

fn convert_tls(t: TLSRawConfig) -> Result<proxy::TLSRawConfig, Error> {
    Ok(proxy::TLSRawConfig {
        ca_file: t.ca_file.map(convert_rawfile).transpose()?,
        cert_file: convert_rawfile(t.cert_file)?,
        key_file: convert_rawfile(t.key_file)?,
    })
}

fn convert_rawfile(f: RawFile) -> Result<proxy::RawFile, Error> {
    Ok(match f {
        RawFile::Path(RawFilePath { value, .. }) => proxy::RawFile::Path(PathBuf::from(value)),
        RawFile::Contents(RawFileContents { value, .. }) => {
            proxy::RawFile::Contents(value.into_bytes())
        }
    })
}

pub(crate) fn get_free_port(ports: Option<Vec<u16>>) -> anyhow::Result<u16> {
    for port in 1025..u16::MAX {
        match &ports {
            None => return Ok(port),
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
    use crate::proxy::config::get_free_port;

    #[test]
    fn test_get_free_port() {
        assert!(get_free_port(Some((u16::MIN..u16::MAX).collect())).is_err());
    }
}
