use std::cell::Cell;
use std::fmt::{self, Display};
use std::io;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use futures::stream::TryStreamExt;
use futures::AsyncReadExt;
use http::{Request, Response};
use hyper::Body;
use rs_tproxy_plugin::header::{RequestHeader, ResponseHeader};
use wasmer_runtime::{func, imports, instantiate, DynFunc, Value};

mod logger;
mod print;

pub enum HandlerName {
    Request,
    Response,
}

#[derive(Debug, Clone)]
pub enum Plugin {
    WASM(Bytes),
}

impl Display for HandlerName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            &HandlerName::Request => f.write_str("handle_request"),
            &HandlerName::Response => f.write_str("handle_response"),
        }
    }
}

impl Plugin {
    async fn read_body(header_map: &http::HeaderMap, body: Body) -> anyhow::Result<Vec<u8>> {
        let size_hint = header_map
            .get(http::header::CONTENT_LENGTH)
            .and_then(|value| std::str::from_utf8(value.as_bytes()).ok()?.parse().ok());
        let mut body_data = match size_hint {
            Some(hint) => Vec::with_capacity(hint),
            None => Vec::new(),
        };
        body.map_err(|err| io::Error::new(io::ErrorKind::Other, err))
            .into_async_read()
            .read_to_end(&mut body_data)
            .await?;
        Ok(body_data)
    }

    pub async fn handle_request(&self, request: Request<Body>) -> anyhow::Result<Request<Body>> {
        let (parts, body) = request.into_parts();
        let header: RequestHeader = (&parts).into();
        let header_data = serde_json::to_vec(&header)?;
        let body_data = Self::read_body(&parts.headers, body).await?;
        let plugin = self.clone();
        let new_body = tokio::task::spawn_blocking(move || {
            plugin.handle_raw(HandlerName::Request, header_data, body_data)
        })
        .await??;
        Ok(Request::from_parts(parts, new_body.into()))
    }

    pub async fn handle_response(&self, request: Response<Body>) -> anyhow::Result<Response<Body>> {
        let (parts, body) = request.into_parts();
        let header: ResponseHeader = (&parts).into();
        let header_data = serde_json::to_vec(&header)?;
        let body_data = Self::read_body(&parts.headers, body).await?;
        let plugin = self.clone();
        let new_body = tokio::task::spawn_blocking(move || {
            plugin.handle_raw(HandlerName::Response, header_data, body_data)
        })
        .await??;
        Ok(Response::from_parts(parts, new_body.into()))
    }

    fn handle_raw(
        self,
        hander_name: HandlerName,
        header: Vec<u8>,
        origin_body: Vec<u8>,
    ) -> anyhow::Result<Vec<u8>> {
        match self {
            Plugin::WASM(data) => Self::handle_wasm(hander_name, &data, &header, origin_body),
        }
    }

    fn handle_wasm(
        hander_name: HandlerName,
        wasm: &[u8],
        header: &[u8],
        origin_body: Vec<u8>,
    ) -> anyhow::Result<Vec<u8>> {
        let ptr = Arc::new(Mutex::new(None));
        let writer = ptr.clone();
        let write_body = move |addr: u32, len: u32| {
            *writer.lock().unwrap() = Some((addr as usize, len as usize))
        };

        let import_object = imports! {
            "env" => {
                "write_body" => func!(write_body),
                "print" => func!(print::print),
                "log_enabled" => func!(logger::log_enabled),
                "log_log" => func!(logger::log_log),
                "log_flush" => func!(logger::log_flush),
            },
        };

        let mut instance =
            instantiate(wasm, &import_object).map_err(|err| anyhow::anyhow!("{}", err))?;

        if instance
            .exports
            .get::<DynFunc>(&hander_name.to_string())
            .is_err()
        {
            return Ok(origin_body);
        }

        let memory = instance.context_mut().memory(0);

        for (byte, cell) in header
            .iter()
            .cloned()
            .zip(memory.view()[0 as usize..(header.len()) as usize].iter())
        {
            cell.set(byte);
        }

        for (byte, cell) in origin_body.iter().cloned().zip(
            memory.view()[header.len() as usize..(header.len() + origin_body.len()) as usize]
                .iter(),
        ) {
            cell.set(byte);
        }

        instance
            .call(
                &hander_name.to_string(),
                &[
                    Value::I64(0),
                    Value::I64(header.len() as _),
                    Value::I64(origin_body.len() as _),
                ],
            )
            .map_err(|err| anyhow::anyhow!("{}", err))?;

        let ptr_ref = *ptr.lock().map_err(|err| anyhow::anyhow!("{}", err))?;
        match ptr_ref {
            None => Ok(Vec::new()),
            Some((addr, len)) => Ok(instance.context().memory(0).view()[addr..(addr + len)]
                .iter()
                .map(Cell::get)
                .collect::<Vec<_>>()),
        }
    }
}

#[cfg(test)]
mod test;

#[cfg(test)]
mod basic_plugin;
