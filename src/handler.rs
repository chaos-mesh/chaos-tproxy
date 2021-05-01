use std::collections::HashMap;
use std::time::Duration;

use anyhow::anyhow;
use futures::TryStreamExt;
use http::header::HeaderMap;
use http::{Method, Request, Response, StatusCode, Uri};
use hyper::Body;
use serde_json::Value;
use tokio::time::sleep;
use tracing::{debug, instrument};
use wildmatch::WildMatch;

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Target {
    Request,
    Response,
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub target: Target,
    pub selector: Selector,
    pub actions: Actions,
}

#[derive(Debug, Clone)]
pub struct Selector {
    pub port: Option<u16>,
    pub path: Option<WildMatch>,
    pub method: Option<Method>,
    pub code: Option<StatusCode>,
    pub request_headers: Option<HeaderMap>,
    pub response_headers: Option<HeaderMap>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Actions {
    pub abort: bool,
    pub delay: Option<Duration>,
    pub replace: Option<ReplaceAction>,
    pub patch: Option<PatchAction>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct PatchAction {
    pub body: Option<PatchBodyAction>,
    pub queries: Option<String>,
    pub headers: Option<HeaderMap>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct ReplaceAction {
    pub path: Option<String>,
    pub method: Option<Method>,
    pub body: Option<Vec<u8>>,
    pub code: Option<StatusCode>,
    pub queries: Option<HashMap<String, String>>,
    pub headers: Option<HeaderMap>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum PatchBodyAction {
    JSON(Value),
}

pub fn select_request(port: u16, request: &Request<Body>, selector: &Selector) -> bool {
    selector.port.iter().all(|p| port == *p)
        && selector
            .path
            .iter()
            .all(|p| p.matches(request.uri().path()))
        && selector.method.iter().all(|m| request.method() == m)
        && selector.request_headers.iter().all(|fields| {
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
    selector: &Selector,
) -> bool {
    selector.port.iter().all(|p| port == *p)
        && selector.path.iter().all(|p| p.matches(uri.path()))
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

async fn read_value(body: &mut Body) -> anyhow::Result<Value> {
    let tmp = std::mem::take(body);
    let data: Vec<u8> = tmp
        .try_fold(vec![], |mut data, seg| {
            data.extend(seg);
            futures::future::ok(data)
        })
        .await?;
    Ok(serde_json::from_slice(&data)?)
}

#[instrument]
pub async fn apply_request_action(
    mut request: Request<Body>,
    actions: &Actions,
) -> anyhow::Result<Request<Body>> {
    if actions.abort {
        return Err(anyhow!("Abort applied"));
    }

    if let Some(delay) = actions.delay {
        sleep(delay).await
    }

    if let Some(replace) = &actions.replace {
        replace_path(request.uri_mut(), replace.path.as_ref())?;

        if let Some(md) = &replace.method {
            *request.method_mut() = md.clone();
        }

        if let Some(data) = &replace.body {
            *request.body_mut() = data.clone().into()
        }

        replace_queries(request.uri_mut(), replace.queries.as_ref())?;

        if let Some(hdrs) = &replace.headers {
            for (key, value) in hdrs {
                request.headers_mut().insert(key, value.clone());
            }
        }
    }

    if let Some(patch) = &actions.patch {
        append_queries(request.uri_mut(), patch.queries.as_ref())?;
        if let Some(hdrs) = &patch.headers {
            for (key, value) in hdrs {
                request.headers_mut().append(key, value.clone());
            }
        }
        if let Some(PatchBodyAction::JSON(value)) = &patch.body {
            let mut data = read_value(&mut request.body_mut()).await?;
            json_patch::merge(&mut data, value);
            *request.body_mut() = serde_json::to_vec(&data)?.into();
        }
    }

    debug!("action applied: {:?}", request);
    Ok(request)
}

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
    actions: &Actions,
) -> anyhow::Result<Response<Body>> {
    if actions.abort {
        return Err(anyhow!("Abort applied"));
    }

    if let Some(delay) = actions.delay {
        sleep(delay).await
    }

    if let Some(replace) = &actions.replace {
        if let Some(co) = replace.code {
            *response.status_mut() = co;
        }

        if let Some(data) = &replace.body {
            *response.body_mut() = data.clone().into()
        }

        if let Some(hdrs) = &replace.headers {
            for (key, value) in hdrs {
                response.headers_mut().insert(key, value.clone());
            }
        }
    }

    if let Some(patch) = &actions.patch {
        if let Some(hdrs) = &patch.headers {
            for (key, value) in hdrs {
                response.headers_mut().append(key, value.clone());
            }
        }
        if let Some(PatchBodyAction::JSON(value)) = &patch.body {
            let mut data = read_value(&mut response.body_mut()).await?;
            json_patch::merge(&mut data, value);
            *response.body_mut() = serde_json::to_vec(&data)?.into();
        }
    }

    debug!("action applied: {:?}", response);
    Ok(response)
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use http::header::{HeaderMap, HeaderName, HeaderValue};
    use http::{Request, Response};
    use hyper::Body;
    use serde_urlencoded::from_str;
    use test_case::test_case;

    use super::{
        append_queries, apply_request_action, apply_response_action, replace_path, replace_queries,
        Actions, PatchAction, ReplaceAction,
    };

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

    #[tokio::test]
    async fn test_apply_request_order() -> anyhow::Result<()> {
        let mut req = Request::new(Body::empty());
        let mut actions = Actions {
            abort: false,
            delay: None,
            replace: None,
            patch: None,
        };

        req = apply_request_action(req, &actions).await?;
        let mut queries = HashMap::new();
        queries.insert("foo".to_string(), "foo".to_string());

        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("foo"),
            HeaderValue::from_static("foo"),
        );

        actions.replace = Some(ReplaceAction {
            path: None,
            method: None,
            body: None,
            code: None,
            queries: Some(queries),
            headers: Some(headers),
        });

        let mut append_headers = HeaderMap::new();
        append_headers.insert(
            HeaderName::from_static("foo"),
            HeaderValue::from_static("bar"),
        );

        actions.patch = Some(PatchAction {
            queries: Some("foo=bar".to_string()),
            headers: Some(append_headers),
            body: None,
        });

        req = apply_request_action(req, &actions).await?;
        assert_eq!(Some("foo=foo&foo=bar"), req.uri().query());
        let foos = req.headers().get_all("foo").into_iter().collect::<Vec<_>>();
        assert_eq!(2, foos.len());
        assert_eq!(HeaderValue::from_static("foo"), foos[0]);
        assert_eq!(HeaderValue::from_static("bar"), foos[1]);
        Ok(())
    }

    #[tokio::test]
    async fn test_apply_response_order() -> anyhow::Result<()> {
        let mut resp = Response::new(Body::empty());
        let mut actions = Actions {
            abort: false,
            delay: None,
            replace: None,
            patch: None,
        };

        resp = apply_response_action(resp, &actions).await?;

        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("foo"),
            HeaderValue::from_static("foo"),
        );

        actions.replace = Some(ReplaceAction {
            path: None,
            method: None,
            body: None,
            code: None,
            queries: None,
            headers: Some(headers),
        });

        let mut append_headers = HeaderMap::new();
        append_headers.insert(
            HeaderName::from_static("foo"),
            HeaderValue::from_static("bar"),
        );

        actions.patch = Some(PatchAction {
            queries: None,
            headers: Some(append_headers),
            body: None,
        });

        resp = apply_response_action(resp, &actions).await?;
        let foos = resp
            .headers()
            .get_all("foo")
            .into_iter()
            .collect::<Vec<_>>();
        assert_eq!(2, foos.len());
        assert_eq!(HeaderValue::from_static("foo"), foos[0]);
        assert_eq!(HeaderValue::from_static("bar"), foos[1]);
        Ok(())
    }
}
