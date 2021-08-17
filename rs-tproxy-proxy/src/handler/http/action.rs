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

        if let Some(body) = &replace.body {
            *request.body_mut() = body.contents.clone().into();
            request.headers_mut().remove(http::header::CONTENT_LENGTH);
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
        if let Some(patch_body) = &patch.body {
            let PatchBodyActionContents::JSON(ref value) = patch_body.contents;
            let mut data = read_value(&mut request.body_mut()).await?;
            json_patch::merge(&mut data, value);
            let merged = serde_json::to_vec(&data)?;
            *request.body_mut() = merged.into();
            request.headers_mut().remove(http::header::CONTENT_LENGTH);
        }

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

        if let Some(body) = &replace.body {
            *response.body_mut() = body.contents.clone().into();
            response.headers_mut().remove(http::header::CONTENT_LENGTH);
        }

        if let Some(hdrs) = &replace.headers {
            for (key, value) in hdrs {
                response.headers_mut().insert(key, value.clone());
            }
        }
    }

    if let Some(patch) = &actions.patch {
        if let Some(patch_body) = &patch.body {
            let PatchBodyActionContents::JSON(ref value) = patch_body.contents;
            let mut data = read_value(&mut response.body_mut()).await?;
            json_patch::merge(&mut data, value);
            let merged = serde_json::to_vec(&data)?;
            *response.body_mut() = merged.into();
            response.headers_mut().remove(http::header::CONTENT_LENGTH);
        }
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
    use crate::handler::http::action::{append_queries, replace_path};

    #[test]
    fn test_append_queries() {
        let mut uri = http::Uri::builder()
            .scheme("https")
            .authority("hyper.rs")
            .path_and_query("/hhh?").build().unwrap();

        append_queries(&mut uri,Some("a=b")).unwrap();
        assert_eq!(&uri.to_string(), "https://hyper.rs/hhh?&a=b");

        append_queries(&mut uri,Some("a=b")).unwrap();
        assert_eq!(&uri.to_string(), "https://hyper.rs/hhh?&a=b&a=b");
    }

    #[test]
    fn test_replace_path() {
        let mut uri = http::Uri::builder()
            .scheme("https")
            .authority("hyper.rs")
            .path_and_query("/hhh/").build().unwrap();

        replace_path(&mut uri,Some("hhh")).unwrap();
        assert_eq!(&uri.to_string(), "https://hyper.rshhh");

        let mut uri = http::Uri::builder()
            .scheme("https")
            .authority("hyper.rs")
            .path_and_query("/ccc?a=b").build().unwrap();

        replace_path(&mut uri,Some("/hhh")).unwrap();
        assert_eq!(&uri.to_string(), "https://hyper.rs/hhh?a=b");
    }

    #[test]
    fn test_replace_queries() {
        //todo
    }
}