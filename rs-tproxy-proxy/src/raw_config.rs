use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::time::Duration;

use anyhow::Error;
use http::header::{HeaderMap, HeaderName};
use http::StatusCode;
use serde::{Deserialize, Serialize};
use wildmatch::WildMatch;

use crate::handler::http::action::{Actions, PatchAction, PatchBodyAction, ReplaceAction};
use crate::handler::http::rule::{Rule, Target};
use crate::handler::http::selector::Selector;
use crate::proxy::http::config::Config;

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct RawConfig {
    pub proxy_ports: Option<String>,
    pub listen_port: u16,
    pub safe_mode: bool,
    pub interface: Option<String>,
    pub rules: Vec<RawRule>,
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

    // [wildcard matches](https://www.wikiwand.com/en/Matching_wildcards)
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
#[serde(tag = "type", content = "value")]
pub enum RawPatchBody {
    // merge patch json as [rfc7396](https://tools.ietf.org/html/rfc7396)
    JSON(String),
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct RawReplaceAction {
    pub path: Option<String>,
    pub method: Option<String>,
    pub body: Option<Vec<u8>>,
    pub code: Option<u16>,
    pub queries: Option<HashMap<String, String>>,
    pub headers: Option<HashMap<String, String>>,
}

impl TryFrom<RawConfig> for Config {
    type Error = Error;

    fn try_from(raw: RawConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            proxy_port: raw.listen_port,
            rules: raw
                .rules
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, Self::Error>>()?,
        })
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
            port: raw.port.clone(),
            path: raw.path.as_ref().map(|p| WildMatch::new(&p)),
            method: raw
                .method
                .as_ref()
                .map(|method| method.parse())
                .transpose()?,
            request_headers: raw
                .request_headers
                .as_ref()
                .map(|headers| -> Result<_, Self::Error> {
                    let mut map = HeaderMap::new();
                    for (key, value) in headers {
                        map.insert(key.parse::<HeaderName>()?, value.parse()?);
                    }
                    Ok(map)
                })
                .transpose()?,
            code: raw.code.clone().map(StatusCode::from_u16).transpose()?,
            response_headers: raw
                .response_headers
                .as_ref()
                .map(|headers| -> Result<_, Self::Error> {
                    let mut map = HeaderMap::new();
                    for (key, value) in headers {
                        map.insert(key.parse::<HeaderName>()?, value.parse()?);
                    }
                    Ok(map)
                })
                .transpose()?,
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
            headers: raw
                .headers
                .map(|headers| -> Result<_, Self::Error> {
                    let mut map = HeaderMap::new();
                    for (key, value) in headers {
                        map.insert(key.parse::<HeaderName>()?, value.parse()?);
                    }
                    Ok(map)
                })
                .transpose()?,
        })
    }
}

impl TryFrom<RawPatchBody> for PatchBodyAction {
    type Error = Error;

    fn try_from(raw: RawPatchBody) -> Result<Self, Self::Error> {
        match raw {
            RawPatchBody::JSON(ref raw) => Ok(PatchBodyAction::JSON(serde_json::from_str(&raw)?)),
        }
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
            body: raw.body,
            code: raw.code.clone().map(StatusCode::from_u16).transpose()?,
            queries: raw.queries,
            headers: raw
                .headers
                .as_ref()
                .map(|headers| -> Result<_, Self::Error> {
                    let mut map = HeaderMap::new();
                    for (key, value) in headers {
                        map.insert(key.parse::<HeaderName>()?, value.parse()?);
                    }
                    Ok(map)
                })
                .transpose()?,
        })
    }
}
