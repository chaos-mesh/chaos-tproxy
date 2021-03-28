use std::collections::HashMap;
use std::time::Duration;

use anyhow::anyhow;
use http::header::HeaderMap;
use http::uri::PathAndQuery;
use http::{Method, Request, Response, StatusCode, Uri};
use hyper::Body;
use tokio::time::sleep;
use tracing::{debug, instrument};

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Rules {
    pub request: Vec<RequestRule>,
    pub response: Vec<ResponseRule>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct RequestRule {
    pub selector: RequestSelector,
    pub action: RequestAction,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct ResponseRule {
    pub selector: ResponseSelector,
    pub action: ResponseAction,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct RequestSelector {
    pub port: Option<u16>,
    pub path: Option<PathAndQuery>,
    pub method: Option<Method>,
    pub headers: Option<HeaderMap>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct ResponseSelector {
    pub port: Option<u16>,
    pub path: Option<PathAndQuery>,
    pub method: Option<Method>,
    pub code: Option<StatusCode>,
    pub request_headers: Option<HeaderMap>,
    pub response_headers: Option<HeaderMap>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum RequestAction {
    Abort,
    Delay(Duration),
    Append {
        queries: Option<String>,
        headers: Option<HeaderMap>,
    },
    Replace {
        path: Option<String>,
        method: Option<Method>,
        body: Option<Vec<u8>>,
        queries: Option<HashMap<String, String>>,
        headers: Option<HeaderMap>,
    },
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum ResponseAction {
    Abort,
    Delay(Duration),
    Append {
        headers: Option<HeaderMap>,
    },
    Replace {
        code: Option<StatusCode>,
        body: Option<Vec<u8>>,
        headers: Option<HeaderMap>,
    },
}

pub fn select_request(port: u16, request: &Request<Body>, selector: &RequestSelector) -> bool {
    selector.port.iter().all(|p| port == *p)
        && selector
            .path
            .iter()
            .all(|p| request.uri().path().starts_with(p.path()))
        && selector.method.iter().all(|m| request.method() == m)
        && selector.headers.iter().all(|fields| {
            fields
                .iter()
                .all(|(header, value)| request.headers().get_all(header).iter().any(|f| f == value))
        })
}

pub fn select_response(
    port: u16,
    uri: &Uri,
    method: &Method,
    request_headers: &HeaderMap,
    response: &Response<Body>,
    selector: &ResponseSelector,
) -> bool {
    selector.port.iter().all(|p| port == *p)
        && selector
            .path
            .iter()
            .all(|p| uri.path().starts_with(p.path()))
        && selector.method.iter().all(|m| method == m)
        && selector.code.iter().all(|code| response.status() == *code)
        && selector.request_headers.iter().all(|fields| {
            fields
                .iter()
                .all(|(header, value)| request_headers.get_all(header).iter().any(|f| f == value))
        })
        && selector.response_headers.iter().all(|fields| {
            fields.iter().all(|(header, value)| {
                response
                    .headers()
                    .get_all(header)
                    .iter()
                    .any(|f| f == value)
            })
        })
}

// TODO: preprocess config to avoid unnecessary error in parsing
#[instrument]
pub async fn apply_request_action(
    mut request: Request<Body>,
    action: &RequestAction,
) -> anyhow::Result<Request<Body>> {
    match action {
        RequestAction::Abort => return Err(anyhow!("Abort applied")),
        RequestAction::Delay(dur) => sleep(*dur).await,
        RequestAction::Append { queries, headers } => {
            if let Some(qs) = &queries {
                // TODO: need test
                let mut parts = request.uri().clone().into_parts();
                let new = if let Some(old) = &parts.path_and_query {
                    if old.query().is_some() {
                        format!("{}&{}", old, qs)
                    } else {
                        format!("{}?{}", old, qs)
                    }
                } else {
                    format!("/?{}", qs)
                };

                parts.path_and_query = Some(new.parse()?);
                *request.uri_mut() = Uri::from_parts(parts)?;
            }

            if let Some(hdrs) = &headers {
                for (key, value) in hdrs {
                    request.headers_mut().append(key, value.clone());
                }
            }
        }
        RequestAction::Replace {
            path,
            method,
            body,
            queries,
            headers,
        } => {
            if let Some(p) = &path {
                // TODO: need test
                let mut parts = request.uri().clone().into_parts();
                if let Some(paq) = parts.path_and_query.as_mut() {
                    *paq = if let Some(q) = paq.query() {
                        format!("{}?{}", p, q).parse()?
                    } else {
                        p.parse()?
                    }
                }
                *request.uri_mut() = Uri::from_parts(parts)?;
            }

            if let Some(md) = method {
                *request.method_mut() = md.clone();
            }

            if let Some(data) = body {
                *request.body_mut() = data.clone().into()
            }

            if let Some(qs) = &queries {
                // TODO: need test
                let mut parts = request.uri().clone().into_parts();
                let old_query = parts
                    .path_and_query
                    .as_ref()
                    .and_then(|paq| paq.query())
                    .unwrap_or("");
                let mut query_map: HashMap<String, String> = serde_urlencoded::from_str(old_query)?;
                query_map.extend(qs.clone());
                let path = parts
                    .path_and_query
                    .as_ref()
                    .map(|paq| paq.path())
                    .unwrap_or("/");
                let paq = format!("{}?{}", path, serde_urlencoded::to_string(&query_map)?);
                parts.path_and_query = Some(paq.parse()?);
                *request.uri_mut() = Uri::from_parts(parts)?;
            }

            if let Some(hdrs) = &headers {
                for (key, value) in hdrs {
                    request.headers_mut().insert(key, value.clone());
                }
            }
        }
    }

    debug!("action applied: {:?}", request);
    Ok(request)
}

// TODO: preprocess config to avoid unnecessary error in parsing
#[instrument]
pub async fn apply_response_action(
    mut response: Response<Body>,
    action: &ResponseAction,
) -> anyhow::Result<Response<Body>> {
    match action {
        ResponseAction::Abort => return Err(anyhow!("Abort applied")),
        ResponseAction::Delay(dur) => sleep(*dur).await,
        ResponseAction::Append { headers } => {
            if let Some(hdrs) = &headers {
                for (key, value) in hdrs {
                    response.headers_mut().append(key, value.clone());
                }
            }
        }
        ResponseAction::Replace {
            code,
            body,
            headers,
        } => {
            if let Some(co) = code {
                *response.status_mut() = *co;
            }

            if let Some(data) = body {
                *response.body_mut() = data.clone().into()
            }

            if let Some(hdrs) = &headers {
                for (key, value) in hdrs {
                    response.headers_mut().insert(key, value.clone());
                }
            }
        }
    }

    debug!("action applied: {:?}", response);
    Ok(response)
}
