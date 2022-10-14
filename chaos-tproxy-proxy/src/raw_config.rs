use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::time::Duration;
use std::{fs, io};

use anyhow::{anyhow, Error};
use http::header::{HeaderMap, HeaderName};
use http::StatusCode;
use rustls::OwnedTrustAnchor;
use rustls_pemfile::{certs, rsa_private_keys};
use serde::{Deserialize, Serialize};
use tokio_rustls::rustls::{Certificate, PrivateKey};
use tokio_rustls::webpki;
use wildmatch::WildMatch;

use crate::handler::http::action::{
    Actions, PatchAction, PatchBodyAction, PatchBodyActionContents, ReplaceAction,
    ReplaceBodyAction,
};
use crate::handler::http::rule::{Rule, Target};
use crate::handler::http::selector::Selector;
use crate::proxy::http::config::{Config, HTTPConfig, TLSConfig};

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize, Default)]
pub struct RawConfig {
    pub proxy_ports: Option<String>,
    pub listen_port: u16,
    pub safe_mode: bool,
    pub rules: Vec<RawRule>,
    pub role: Option<Role>,
    pub tls: Option<TLSRawConfig>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub enum Role {
    Client(Vec<Ipv4Addr>),
    Server(Vec<Ipv4Addr>),
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum RawFile {
    Path(PathBuf),
    Contents(Vec<u8>),
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize, Default)]
pub struct TLSRawConfig {
    pub ca_file: Option<RawFile>,
    pub cert_file: RawFile,
    pub key_file: RawFile,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct RawRule {
    pub target: RawTarget,
    pub selector: RawSelector,
    pub actions: RawActions,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub enum RawTarget {
    Request,
    Response,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct RawSelector {
    pub port: Option<u16>,
    /// Mathc path of `Uri` with wildcard matches.
    ///
    /// Both relative and absolute URIs contain a path component, though it
    /// might be the empty string. The path component is **case sensitive**.
    ///
    /// ```notrust
    /// abc://username:password@example.com:123/path/data?key=value&key2=value2#fragid1
    ///                                        |--------|
    ///                                             |
    ///                                           path
    /// ```
    /// [wildcard matches](https://www.wikiwand.com/en/Matching_wildcards)
    pub path: Option<String>,
    pub method: Option<String>,
    pub code: Option<u16>,
    pub request_headers: Option<HashMap<String, String>>,
    pub response_headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct RawActions {
    pub abort: Option<bool>,
    #[serde(default)]
    #[serde(with = "humantime_serde")]
    pub delay: Option<Duration>,
    pub replace: Option<RawReplaceAction>,
    pub patch: Option<RawPatchAction>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct RawPatchAction {
    // patch body
    pub body: Option<RawPatchBody>,

    // append queries by key-value
    pub queries: Option<Vec<(String, String)>>,

    // append headers by key-value
    pub headers: Option<Vec<(String, String)>>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct RawPatchBody {
    // the contents of body patch
    pub contents: RawPatchBodyContents,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum RawPatchBodyContents {
    // merge patch json as [rfc7396](https://tools.ietf.org/html/rfc7396)
    JSON(String),
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct RawReplaceAction {
    pub path: Option<String>,
    pub method: Option<String>,
    pub body: Option<RawReplaceBody>,
    pub code: Option<u16>,
    pub queries: Option<HashMap<String, String>>,
    pub headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct RawReplaceBody {
    // the contents of body patch
    pub contents: RawReplaceBodyContents,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum RawReplaceBodyContents {
    // replace body with text
    TEXT(String),

    // replace body with base64 encoded data
    BASE64(String),
}

pub(crate) fn try_from_hash_map(
    t: Option<HashMap<String, String>>,
) -> Result<Option<HeaderMap>, anyhow::Error> {
    t.as_ref()
        .map(|headers| -> Result<_, anyhow::Error> {
            headers
                .try_into()
                .map_err(|e: http::Error| -> anyhow::Error { anyhow!(e) })
        })
        .transpose()
}

pub(crate) fn try_from_vec(
    t: Option<Vec<(String, String)>>,
) -> Result<Option<HeaderMap>, anyhow::Error> {
    t.map(|headers| -> Result<_, anyhow::Error> {
        let mut map = HeaderMap::new();
        for (key, value) in headers {
            map.insert(key.parse::<HeaderName>()?, value.parse()?);
        }
        Ok(map)
    })
    .transpose()
}

impl Default for RawFile {
    fn default() -> Self {
        RawFile::Contents(Default::default())
    }
}

impl TryFrom<RawFile> for Vec<u8> {
    type Error = Error;

    fn try_from(value: RawFile) -> Result<Self, Self::Error> {
        match value {
            RawFile::Contents(c) => Ok(c),
            RawFile::Path(p) => Ok(fs::read(p)?),
        }
    }
}

impl TryFrom<RawConfig> for Config {
    type Error = Error;

    fn try_from(raw: RawConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            http_config: HTTPConfig {
                proxy_port: raw.listen_port,
                role: raw.role,
                rules: raw
                    .rules
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<Vec<_>, Self::Error>>()?,
            },

            tls_config: match raw.tls {
                None => None,
                Some(tls) => Some(tls.try_into()?),
            },
        })
    }
}

impl TryFrom<TLSRawConfig> for TLSConfig {
    type Error = Error;

    fn try_from(raw: TLSRawConfig) -> Result<Self, Self::Error> {
        let certs = certs(&mut &*Vec::<u8>::try_from(raw.cert_file)?)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid cert"))
            .map(|mut certs| certs.drain(..).map(Certificate).collect())?;
        let keys: Vec<PrivateKey> = rsa_private_keys(&mut &*Vec::<u8>::try_from(raw.key_file)?)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid key"))
            .map(|mut keys| keys.drain(..).map(PrivateKey).collect())?;

        if keys.is_empty() {
            return Err(anyhow!("empty key"));
        }
        let key = keys[0].clone();

        let mut root_cert_store = rustls::RootCertStore::empty();
        if let Some(cafile) = raw.ca_file {
            let certs = rustls_pemfile::certs(&mut &*Vec::<u8>::try_from(cafile)?)?;
            let trust_anchors = certs.iter().map(|cert| {
                let ta = webpki::TrustAnchor::try_from_cert_der(&cert[..]).unwrap();
                OwnedTrustAnchor::from_subject_spki_name_constraints(
                    ta.subject,
                    ta.spki,
                    ta.name_constraints,
                )
            });
            root_cert_store.add_server_trust_anchors(trust_anchors);
        } else {
            root_cert_store.add_server_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.0.iter().map(
                |ta| {
                    OwnedTrustAnchor::from_subject_spki_name_constraints(
                        ta.subject,
                        ta.spki,
                        ta.name_constraints,
                    )
                },
            ));
        }

        let tls_config = Self {
            tls_client_config: rustls::ClientConfig::builder()
                .with_safe_defaults()
                .with_root_certificates(root_cert_store)
                .with_no_client_auth(),
            tls_server_config: rustls::ServerConfig::builder()
                .with_safe_defaults()
                .with_no_client_auth()
                .with_single_cert(certs, key)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?,
        };
        Ok(tls_config)
    }
}

impl TryFrom<RawRule> for Rule {
    type Error = Error;

    fn try_from(rule: RawRule) -> Result<Self, Self::Error> {
        Ok(Self {
            target: rule.target.into(),
            selector: rule.selector.try_into()?,
            actions: rule.actions.try_into()?,
        })
    }
}

impl From<RawTarget> for Target {
    fn from(target: RawTarget) -> Self {
        match target {
            RawTarget::Request => Target::Request,
            RawTarget::Response => Target::Response,
        }
    }
}

impl TryFrom<RawSelector> for Selector {
    type Error = Error;

    fn try_from(raw: RawSelector) -> Result<Self, Self::Error> {
        Ok(Self {
            port: raw.port,
            path: raw.path.as_ref().map(|p| WildMatch::new(p)),
            method: raw
                .method
                .as_ref()
                .map(|method| method.parse())
                .transpose()?,
            request_headers: try_from_hash_map(raw.request_headers)?,
            code: raw.code.map(StatusCode::from_u16).transpose()?,
            response_headers: try_from_hash_map(raw.response_headers)?,
        })
    }
}

impl TryFrom<RawActions> for Actions {
    type Error = Error;

    fn try_from(raw: RawActions) -> Result<Self, Self::Error> {
        Ok(Self {
            abort: raw.abort.unwrap_or(false),
            delay: raw.delay,
            replace: raw.replace.map(TryInto::try_into).transpose()?,
            patch: raw.patch.map(TryInto::try_into).transpose()?,
        })
    }
}

impl TryFrom<RawPatchAction> for PatchAction {
    type Error = Error;

    fn try_from(raw: RawPatchAction) -> Result<Self, Self::Error> {
        Ok(Self {
            body: raw.body.map(TryInto::try_into).transpose()?,
            queries: raw.queries.map(serde_urlencoded::to_string).transpose()?,
            headers: try_from_vec(raw.headers)?,
        })
    }
}

impl TryFrom<RawPatchBodyContents> for PatchBodyActionContents {
    type Error = Error;

    fn try_from(raw: RawPatchBodyContents) -> Result<Self, Self::Error> {
        match raw {
            RawPatchBodyContents::JSON(ref raw) => {
                Ok(PatchBodyActionContents::JSON(serde_json::from_str(raw)?))
            }
        }
    }
}

impl TryFrom<RawPatchBody> for PatchBodyAction {
    type Error = Error;

    fn try_from(raw: RawPatchBody) -> Result<Self, Self::Error> {
        Ok(Self {
            contents: raw.contents.try_into()?,
        })
    }
}

impl TryFrom<RawReplaceBody> for ReplaceBodyAction {
    type Error = Error;

    fn try_from(raw: RawReplaceBody) -> Result<Self, Self::Error> {
        Ok(Self {
            contents: match raw.contents {
                RawReplaceBodyContents::TEXT(text) => text.into_bytes(),
                RawReplaceBodyContents::BASE64(encoded) => base64::decode(&encoded)?,
            },
        })
    }
}

impl TryFrom<RawReplaceAction> for ReplaceAction {
    type Error = Error;

    fn try_from(raw: RawReplaceAction) -> Result<Self, Self::Error> {
        Ok(Self {
            path: raw.path,
            method: raw
                .method
                .as_ref()
                .map(|method| method.parse())
                .transpose()?,
            body: raw.body.map(TryFrom::try_from).transpose()?,
            code: raw.code.map(StatusCode::from_u16).transpose()?,
            queries: raw.queries,
            headers: try_from_hash_map(raw.headers)?,
        })
    }
}
