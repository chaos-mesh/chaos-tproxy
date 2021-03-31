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

#[instrument]
pub async fn apply_request_action(
    mut request: Request<Body>,
    action: &RequestAction,
) -> anyhow::Result<Request<Body>> {
    match action {
        RequestAction::Abort => return Err(anyhow!("Abort applied")),
        RequestAction::Delay(dur) => sleep(*dur).await,
        RequestAction::Append { queries, headers } => {
            append_queries(request.uri_mut(), queries.as_ref())?;
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
            replace_path(request.uri_mut(), path.as_ref())?;

            if let Some(md) = method {
                *request.method_mut() = md.clone();
            }

            if let Some(data) = body {
                *request.body_mut() = data.clone().into()
            }

            replace_queries(request.uri_mut(), queries.as_ref())?;

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

// TODO: need test
fn append_queries<S: AsRef<str>>(uri: &mut Uri, raw_queries: Option<S>) -> anyhow::Result<()> {
    let queries = raw_queries.as_ref().map(AsRef::as_ref).unwrap_or("");
    if !queries.is_empty() {
        let mut parts = uri.clone().into_parts();
        let new = if let Some(old) = &parts.path_and_query {
            if old.query().is_some() {
                format!("{}&{}", old, queries)
            } else {
                format!("{}?{}", old, queries)
            }
        } else {
            format!("/?{}", queries)
        };

        parts.path_and_query = Some(new.parse()?);
        *uri = Uri::from_parts(parts)?;
    }
    Ok(())
}

// TODO: need test
fn replace_path<S: AsRef<str>>(uri: &mut Uri, raw_path: Option<S>) -> anyhow::Result<()> {
    if let Some(p) = raw_path {
        let path = match p.as_ref() {
            "" => "/",
            s => s,
        };

        let mut parts = uri.clone().into_parts();
        if let Some(paq) = parts.path_and_query.as_mut() {
            *paq = if let Some(q) = paq.query() {
                format!("{}?{}", path, q).parse()?
            } else {
                path.parse()?
            }
        }
        *uri = Uri::from_parts(parts)?;
    }
    Ok(())
}

// TODO: need test
fn replace_queries(uri: &mut Uri, queries: Option<&HashMap<String, String>>) -> anyhow::Result<()> {
    if let Some(qs) = queries {
        let mut parts = uri.clone().into_parts();
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
        let paq = match serde_urlencoded::to_string(&query_map)?.as_str() {
            "" => path.parse()?,
            q => format!("{}?{}", path, q).parse()?,
        };

        parts.path_and_query = Some(paq);
        *uri = Uri::from_parts(parts)?;
    }
    Ok(())
}

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

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use serde_urlencoded::from_str;
    use test_case::test_case;

    use super::{append_queries, replace_path, replace_queries};

    #[test_case("/", None => "/")]
    #[test_case("/", Some("") => "/")]
    #[test_case("/", Some("foo=bar") => "/?foo=bar")]
    #[test_case("/lgtm", Some("foo=bar") => "/lgtm?foo=bar")]
    #[test_case("/?os=linux", None => "/?os=linux")]
    #[test_case("/?os=linux", Some("") => "/?os=linux")]
    #[test_case("/?os=linux", Some("foo=bar") => "/?os=linux&foo=bar")]
    #[test_case("/lgtm?os=linux", Some("foo=bar") => "/lgtm?os=linux&foo=bar")]
    #[test_case("/lgtm?os=linux&foo=foo", Some("foo=bar") => "/lgtm?os=linux&foo=foo&foo=bar")]
    #[test_case("/lgtm?os=linux&foo=foo", Some("foo=bar&os=windows") => "/lgtm?os=linux&foo=foo&foo=bar&os=windows")]
    fn test_append_queries(raw_uri: &str, queries: Option<&str>) -> String {
        let uri_parse = raw_uri.parse();
        assert!(uri_parse.is_ok());
        let mut uri = uri_parse.unwrap();
        assert!(append_queries(&mut uri, queries).is_ok());
        uri.to_string()
    }

    #[test_case("/", None => "/")]
    #[test_case("/", Some("") => "/")]
    #[test_case("/", Some("foo=bar") => "/?foo=bar")]
    #[test_case("/lgtm", Some("foo=bar") => "/lgtm?foo=bar")]
    #[test_case("/?os=linux", None => "/?os=linux")]
    #[test_case("/?os=linux", Some("") => "/?os=linux")]
    #[test_case("/?foo=foo", Some("foo=bar") => "/?foo=bar")]
    #[test_case("/?foo=foo&foo=foo2", Some("foo=bar") => "/?foo=bar")]
    fn test_replace_queries(raw_uri: &str, queries: Option<&str>) -> String {
        let uri_parse = raw_uri.parse();
        assert!(uri_parse.is_ok());
        let mut uri = uri_parse.unwrap();
        let queries_parse = queries.map(from_str).transpose();
        assert!(queries_parse.is_ok());
        assert!(replace_queries(&mut uri, queries_parse.unwrap().as_ref()).is_ok());
        uri.to_string()
    }

    #[test_case("/?os=linux", Some("foo=bar"), "os=linux&foo=bar")]
    #[test_case("/?os=linux&foo=foo", Some("foo=bar"), "os=linux&foo=bar")]
    #[test_case("/?os=linux&foo=foo", Some("foo=bar&os=windows"), "foo=bar&os=windows")]
    fn test_replace_queries_pro(raw_uri: &str, queries: Option<&str>, expected_queries: &str) {
        let uri_parse = raw_uri.parse();
        assert!(uri_parse.is_ok());
        let mut uri = uri_parse.unwrap();
        let queries_parse = queries.map(from_str).transpose();
        assert!(queries_parse.is_ok());
        assert!(replace_queries(&mut uri, queries_parse.unwrap().as_ref()).is_ok());

        let query_map: HashMap<String, String> =
            serde_urlencoded::from_str(uri.query().unwrap_or("")).unwrap();

        let expected_query_map_parse: Result<HashMap<String, String>, _> =
            serde_urlencoded::from_str(expected_queries);
        assert!(expected_query_map_parse.is_ok());
        let expected_query_map = expected_query_map_parse.unwrap();

        assert_eq!(query_map.len(), expected_query_map.len());
        assert!(query_map.iter().all(|(k, v)| {
            let expected_value = expected_query_map.get(k);
            expected_value.is_some() && expected_value.unwrap() == v
        }));
    }

    #[test_case("/", None => "/")]
    #[test_case("/", Some("") => "/")]
    #[test_case("/lgtm?foo=bar", Some("") => "/?foo=bar")]
    #[test_case("/?os=linux", None => "/?os=linux")]
    #[test_case("/pull/1/lgtm?foo=bar", Some("/pull/2/lgtm") => "/pull/2/lgtm?foo=bar")]
    fn test_replace_path(raw_uri: &str, path: Option<&str>) -> String {
        let uri_parse = raw_uri.parse();
        assert!(uri_parse.is_ok());
        let mut uri = uri_parse.unwrap();
        assert!(replace_path(&mut uri, path).is_ok());
        uri.to_string()
    }
}
