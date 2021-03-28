use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::time::Duration;

use anyhow::{anyhow, Error};
use http::header::{HeaderMap, HeaderName};
use http::StatusCode;
use multimap::MultiMap;
use serde::{Deserialize, Serialize};

use crate::handler::{
    RequestAction, RequestRule, RequestSelector, ResponseAction, ResponseRule, ResponseSelector,
    Rules,
};
use crate::tproxy::config::Config;

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct RawConfig {
    pub listen_port: Option<u16>,
    pub proxy_ports: Vec<u16>,
    pub proxy_mark: Option<i32>,
    pub ignore_mark: Option<i32>,
    pub route_table: Option<u8>,
    pub rules: RawRules,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct RawRules {
    pub request: Option<Vec<RawRequestRule>>,
    pub response: Option<Vec<RawResponseRule>>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct RawRequestRule {
    pub selector: RawRequestSelector,
    pub action: RawRequestAction,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct RawResponseRule {
    pub selector: RawResponseSelector,
    pub action: RawResponseAction,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct RawRequestSelector {
    pub path: Option<String>,
    pub method: Option<String>,
    pub headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct RawResponseSelector {
    pub path: Option<String>,
    pub method: Option<String>,
    pub code: Option<u16>,
    pub request_headers: Option<HashMap<String, String>>,
    pub response_headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RawRequestAction {
    Abort,
    Delay(#[serde(with = "humantime_serde")] Duration),
    Append {
        queries: Option<MultiMap<String, String>>,
        headers: Option<Vec<(String, String)>>,
    },
    Replace {
        path: Option<String>,
        method: Option<String>,
        body: Option<Vec<u8>>,
        queries: Option<HashMap<String, String>>,
        headers: Option<HashMap<String, String>>,
    },
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RawResponseAction {
    Abort,
    Delay(#[serde(with = "humantime_serde")] Duration),
    Append {
        headers: Option<Vec<(String, String)>>,
    },
    Replace {
        code: Option<u16>,
        body: Option<Vec<u8>>,
        headers: Option<HashMap<String, String>>,
    },
}

impl TryFrom<RawConfig> for Config {
    type Error = Error;

    fn try_from(raw: RawConfig) -> Result<Self, Self::Error> {
        if raw.proxy_ports.is_empty() {
            return Err(anyhow!("proxy ports cannot be empty"));
        }

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
            proxy_ports: raw
                .proxy_ports
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
            proxy_mark,
            ignore_mark,
            route_table,
            rules: raw.rules.try_into()?,
        })
    }
}

impl TryFrom<RawRules> for Rules {
    type Error = Error;

    fn try_from(RawRules { request, response }: RawRules) -> Result<Self, Self::Error> {
        Ok(Self {
            request: request
                .unwrap_or_else(Vec::new)
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, Self::Error>>()?,
            response: response
                .unwrap_or_else(Vec::new)
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, Self::Error>>()?,
        })
    }
}

impl TryFrom<RawRequestRule> for RequestRule {
    type Error = Error;

    fn try_from(RawRequestRule { selector, action }: RawRequestRule) -> Result<Self, Self::Error> {
        Ok(Self {
            selector: selector.try_into()?,
            action: action.try_into()?,
        })
    }
}

impl TryFrom<RawResponseRule> for ResponseRule {
    type Error = Error;

    fn try_from(
        RawResponseRule { selector, action }: RawResponseRule,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            selector: selector.try_into()?,
            action: action.try_into()?,
        })
    }
}

impl TryFrom<RawRequestSelector> for RequestSelector {
    type Error = Error;

    fn try_from(raw: RawRequestSelector) -> Result<Self, Self::Error> {
        Ok(Self {
            path: raw.path.as_ref().map(|path| path.parse()).transpose()?,
            method: raw
                .method
                .as_ref()
                .map(|method| method.parse())
                .transpose()?,
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

impl TryFrom<RawRequestAction> for RequestAction {
    type Error = Error;

    fn try_from(raw: RawRequestAction) -> Result<Self, Self::Error> {
        Ok(match raw {
            RawRequestAction::Abort => RequestAction::Abort,
            RawRequestAction::Delay(dur) => RequestAction::Delay(dur),
            RawRequestAction::Append { queries, headers } => RequestAction::Append {
                queries: queries.map(serde_urlencoded::to_string).transpose()?,
                headers: headers
                    .map(|headers| -> Result<_, Self::Error> {
                        let mut map = HeaderMap::new();
                        for (key, value) in headers {
                            map.insert(key.parse::<HeaderName>()?, value.parse()?);
                        }
                        Ok(map)
                    })
                    .transpose()?,
            },
            RawRequestAction::Replace {
                path,
                method,
                body,
                queries,
                headers,
            } => RequestAction::Replace {
                path: path.as_ref().map(|path| path.parse()).transpose()?,
                method: method.as_ref().map(|method| method.parse()).transpose()?,
                body: body,
                queries: queries,
                headers: headers
                    .as_ref()
                    .map(|headers| -> Result<_, Self::Error> {
                        let mut map = HeaderMap::new();
                        for (key, value) in headers {
                            map.insert(key.parse::<HeaderName>()?, value.parse()?);
                        }
                        Ok(map)
                    })
                    .transpose()?,
            },
        })
    }
}
impl TryFrom<RawResponseSelector> for ResponseSelector {
    type Error = Error;

    fn try_from(raw: RawResponseSelector) -> Result<Self, Self::Error> {
        Ok(Self {
            path: raw.path.as_ref().map(|path| path.parse()).transpose()?,
            method: raw
                .method
                .as_ref()
                .map(|method| method.parse())
                .transpose()?,
            code: raw.code.clone().map(StatusCode::from_u16).transpose()?,
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

impl TryFrom<RawResponseAction> for ResponseAction {
    type Error = Error;

    fn try_from(raw: RawResponseAction) -> Result<Self, Self::Error> {
        Ok(match raw {
            RawResponseAction::Abort => ResponseAction::Abort,
            RawResponseAction::Delay(dur) => ResponseAction::Delay(dur),
            RawResponseAction::Append { headers } => ResponseAction::Append {
                headers: headers
                    .map(|headers| -> Result<_, Self::Error> {
                        let mut map = HeaderMap::new();
                        for (key, value) in headers {
                            map.insert(key.parse::<HeaderName>()?, value.parse()?);
                        }
                        Ok(map)
                    })
                    .transpose()?,
            },
            RawResponseAction::Replace {
                code,
                body,
                headers,
            } => ResponseAction::Replace {
                code: code.clone().map(StatusCode::from_u16).transpose()?,
                body: body,
                headers: headers
                    .as_ref()
                    .map(|headers| -> Result<_, Self::Error> {
                        let mut map = HeaderMap::new();
                        for (key, value) in headers {
                            map.insert(key.parse::<HeaderName>()?, value.parse()?);
                        }
                        Ok(map)
                    })
                    .transpose()?,
            },
        })
    }
}
