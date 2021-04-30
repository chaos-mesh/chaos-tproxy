use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::time::Duration;

use anyhow::{anyhow, Error};
use http::header::{HeaderMap, HeaderName};
use http::StatusCode;
use serde::{Deserialize, Serialize};

use crate::handler::{Actions, AppendAction, ReplaceAction, Rule, Selector, Target};
use crate::tproxy::config::Config;

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize, Default)]
pub struct RawConfig {
    pub listen_port: Option<u16>,
    pub proxy_ports: Vec<u16>,
    pub proxy_mark: Option<i32>,
    pub ignore_mark: Option<i32>,
    pub route_table: Option<u8>,
    pub rules: Option<Vec<RawRule>>,
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
    pub append: Option<RawAppendAction>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct RawAppendAction {
    pub queries: Option<Vec<(String, String)>>,
    pub headers: Option<Vec<(String, String)>>,
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
        let proxy_mark = raw.proxy_mark.unwrap_or(1);
        let ignore_mark = raw.ignore_mark.unwrap_or(255);
        let route_table = raw.route_table.unwrap_or(100);

        if proxy_mark == ignore_mark {
            return Err(anyhow!(
                "proxy mark cannot be the same with ignore mark: {}={}",
                proxy_mark,
                ignore_mark
            ));
        }

        if route_table == 0 || route_table > 252 {
            return Err(anyhow!("invalid route table: table({})", route_table));
        }

        Ok(Self {
            listen_port: raw.listen_port.unwrap_or(0),
            proxy_ports: if raw.proxy_ports.is_empty() {
                None
            } else {
                Some(
                    raw.proxy_ports
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                )
            },
            proxy_mark,
            ignore_mark,
            route_table,
            rules: raw
                .rules
                .unwrap_or_default()
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
            path: raw.path.as_ref().map(|path| path.parse()).transpose()?,
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
            append: raw.append.map(TryInto::try_into).transpose()?,
        })
    }
}

impl TryFrom<RawAppendAction> for AppendAction {
    type Error = Error;

    fn try_from(raw: RawAppendAction) -> Result<Self, Self::Error> {
        Ok(Self {
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
