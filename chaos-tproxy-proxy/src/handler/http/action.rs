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
    pub body: Option<ReplaceBodyAction>,
    pub code: Option<StatusCode>,
    pub queries: Option<HashMap<String, String>>,
    pub headers: Option<HeaderMap>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct ReplaceBodyAction {
    pub contents: Vec<u8>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct PatchBodyAction {
    pub contents: PatchBodyActionContents,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum PatchBodyActionContents {
    JSON(Value),
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

/// apply_request_action would inject chaos actions into the given request.
/// TODO(@STRRL): refactor this function, it is NOT extensible with more actions.
#[instrument]
pub async fn apply_request_action(
    mut request: Request<Body>,
    actions: &Actions,
) -> anyhow::Result<Request<Body>> {
    // abort the request
    if actions.abort {
        return Err(anyhow!("Abort applied"));
    }

    // delay the request
    if let Some(delay) = actions.delay {
        sleep(delay).await
    }

    if let Some(replace) = &actions.replace {
        // replace the request URL
        replace_path(request.uri_mut(), replace.path.as_ref())?;

        if let Some(md) = &replace.method {
            // replace the request method
            *request.method_mut() = md.clone();
        }

        if let Some(body) = &replace.body {
            // replace the request body
            *request.body_mut() = body.contents.clone().into();
            request.headers_mut().remove(http::header::CONTENT_LENGTH);
        }

        // replace request query parameters
        replace_queries(request.uri_mut(), replace.queries.as_ref())?;

        if let Some(hdrs) = &replace.headers {
            // replace the request headers
            for (key, value) in hdrs {
                request.headers_mut().insert(key, value.clone());
            }
        }
    }

    if let Some(patch) = &actions.patch {
        // append request query parameters
        append_queries(request.uri_mut(), patch.queries.as_ref())?;

        // patch request body with JSON Patch
        if let Some(patch_body) = &patch.body {
            let PatchBodyActionContents::JSON(ref value) = patch_body.contents;
            let mut data = read_value(request.body_mut()).await?;
            json_patch::merge(&mut data, value);
            let merged = serde_json::to_vec(&data)?;
            *request.body_mut() = merged.into();
            request.headers_mut().remove(http::header::CONTENT_LENGTH);
        }

        // patch headers
        if let Some(hdrs) = &patch.headers {
            for (key, value) in hdrs {
                request.headers_mut().append(key, value.clone());
            }
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

/// apply_response_action would inject chaos actions into the given response.
/// TODO(@STRRL): refactor this function, it is NOT extensible with more actions.
#[instrument]
pub async fn apply_response_action(
    mut response: Response<Body>,
    actions: &Actions,
) -> anyhow::Result<Response<Body>> {
    // abort the response
    if actions.abort {
        return Err(anyhow!("Abort applied"));
    }

    // delay the response
    if let Some(delay) = actions.delay {
        sleep(delay).await
    }

    if let Some(replace) = &actions.replace {
        // replace the response code
        if let Some(co) = replace.code {
            *response.status_mut() = co;
        }

        // replace the response body
        if let Some(body) = &replace.body {
            *response.body_mut() = body.contents.clone().into();
            response.headers_mut().remove(http::header::CONTENT_LENGTH);
        }

        // replace the response header
        if let Some(hdrs) = &replace.headers {
            for (key, value) in hdrs {
                response.headers_mut().insert(key, value.clone());
            }
        }
    }

    if let Some(patch) = &actions.patch {
        // patch response body with JSON Patch
        if let Some(patch_body) = &patch.body {
            let PatchBodyActionContents::JSON(ref value) = patch_body.contents;
            let mut data = read_value(response.body_mut()).await?;
            json_patch::merge(&mut data, value);
            let merged = serde_json::to_vec(&data)?;
            *response.body_mut() = merged.into();
            response.headers_mut().remove(http::header::CONTENT_LENGTH);
        }
        // patch headers
        if let Some(hdrs) = &patch.headers {
            for (key, value) in hdrs {
                response.headers_mut().append(key, value.clone());
            }
        }
    }

    debug!("action applied: {:?}", response);
    Ok(response)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::convert::TryInto;

    use http::{Request, Response, Uri};
    use hyper::Body;
    use serde_json::json;

    use super::{
        append_queries, apply_request_action, apply_response_action, replace_path, replace_queries,
        Actions,
    };
    use crate::raw_config::RawActions;

    #[test]
    fn test_append_queries() {
        let mut uri = http::Uri::builder()
            .scheme("https")
            .authority("hyper.rs")
            .path_and_query("/hhh?")
            .build()
            .unwrap();

        append_queries(&mut uri, Some("a=b")).unwrap();
        assert_eq!(&uri.to_string(), "https://hyper.rs/hhh?&a=b");

        append_queries(&mut uri, Some("a=b")).unwrap();
        assert_eq!(&uri.to_string(), "https://hyper.rs/hhh?&a=b&a=b");
    }

    #[test]
    fn test_replace_path() {
        let mut uri = http::Uri::builder()
            .scheme("https")
            .authority("hyper.rs")
            .path_and_query("/hhh/")
            .build()
            .unwrap();

        replace_path(&mut uri, Some("hhh")).unwrap();
        assert_eq!(&uri.to_string(), "https://hyper.rshhh");

        let mut uri = http::Uri::builder()
            .scheme("https")
            .authority("hyper.rs")
            .path_and_query("/ccc?a=b")
            .build()
            .unwrap();

        replace_path(&mut uri, Some("/hhh")).unwrap();
        assert_eq!(&uri.to_string(), "https://hyper.rs/hhh?a=b");
    }

    #[tokio::test]
    async fn request_patch_appends_configured_duplicate_headers() {
        let raw: RawActions = serde_json::from_value(json!({
            "patch": {
                "headers": [["X-Tag", "first"], ["x-tag", "second"]]
            }
        }))
        .unwrap();
        let actions: Actions = raw.try_into().unwrap();
        let request = Request::builder()
            .header("x-tag", "original")
            .body(Body::empty())
            .unwrap();

        let request = apply_request_action(request, &actions).await.unwrap();

        assert_eq!(
            request
                .headers()
                .get_all("x-tag")
                .iter()
                .collect::<Vec<_>>(),
            ["original", "first", "second"]
        );
    }

    #[tokio::test]
    async fn response_patch_appends_configured_duplicate_headers() {
        let raw: RawActions = serde_json::from_value(json!({
            "patch": {
                "headers": [["Set-Cookie", "first=1"], ["set-cookie", "second=2"]]
            }
        }))
        .unwrap();
        let actions: Actions = raw.try_into().unwrap();
        let response = Response::builder()
            .header("set-cookie", "original=0")
            .body(Body::empty())
            .unwrap();

        let response = apply_response_action(response, &actions).await.unwrap();

        assert_eq!(
            response
                .headers()
                .get_all("set-cookie")
                .iter()
                .collect::<Vec<_>>(),
            ["original=0", "first=1", "second=2"]
        );
    }

    #[test]
    fn replace_queries_preserves_unrelated_pairs_and_replaces_all_target_values() {
        let mut uri: Uri = "https://example.com:8443/items?tag=a&page=1&tag=b&page=3&empty=&q=hello+world&encoded=a%26b&encoded=a%3Db&%70age=4"
            .parse()
            .unwrap();
        let replacements = HashMap::from([
            ("page".into(), "2".into()),
            ("new key".into(), "a+b &=/".into()),
        ]);

        replace_queries(&mut uri, Some(&replacements)).unwrap();

        assert_eq!(uri.scheme_str(), Some("https"));
        assert_eq!(uri.authority().unwrap().as_str(), "example.com:8443");
        assert_eq!(uri.path(), "/items");
        let pairs: Vec<(String, String)> =
            serde_urlencoded::from_str(uri.query().unwrap()).unwrap();
        assert_eq!(
            &pairs[..6],
            &[
                ("tag".into(), "a".into()),
                ("tag".into(), "b".into()),
                ("empty".into(), "".into()),
                ("q".into(), "hello world".into()),
                ("encoded".into(), "a&b".into()),
                ("encoded".into(), "a=b".into()),
            ]
        );
        let mut replaced = pairs[6..].to_vec();
        replaced.sort();
        assert_eq!(
            replaced,
            [
                ("new key".into(), "a+b &=/".into()),
                ("page".into(), "2".into())
            ]
        );
    }

    #[test]
    fn replace_queries_adds_pairs_without_an_existing_query() {
        let mut uri: Uri = "/items".parse().unwrap();
        let replacements = HashMap::from([("page".into(), "2".into())]);

        replace_queries(&mut uri, Some(&replacements)).unwrap();

        assert_eq!(uri, "/items?page=2");
    }

    #[test]
    fn replace_queries_with_no_replacements_leaves_uri_unchanged() {
        let empty = HashMap::new();
        for replacements in [None, Some(&empty)] {
            for original in [
                "/items?tag=a&tag=b&q=hello%20world&empty=",
                "/items?",
                "/items",
            ] {
                let mut uri: Uri = original.parse().unwrap();

                replace_queries(&mut uri, replacements).unwrap();

                assert_eq!(uri.to_string(), original);
            }
        }
    }

    #[tokio::test]
    async fn request_replaces_queries_before_appending_patch_pairs() {
        let raw: RawActions = serde_json::from_value(json!({
            "replace": {"queries": {"page": "2"}},
            "patch": {"queries": [["tag", "c"], ["page", "4"]]}
        }))
        .unwrap();
        let actions: Actions = raw.try_into().unwrap();
        let request = Request::builder()
            .uri("/items?tag=a&page=1&tag=b&page=3")
            .body(Body::empty())
            .unwrap();

        let request = apply_request_action(request, &actions).await.unwrap();

        assert_eq!(request.uri(), "/items?tag=a&tag=b&page=2&tag=c&page=4");
    }
}
