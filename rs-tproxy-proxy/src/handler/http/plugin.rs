use std::cell::Cell;
use std::collections::HashMap;
use std::fmt::{self, Display};
use std::sync::{Arc, Mutex};

use http::{request, response, Request, Response};
use hyper::Body;
use serde::{Deserialize, Serialize};
use wasmer_runtime::{func, imports, instantiate, DynFunc, Value};

pub enum HandlerName {
    Request,
    Response,
}

#[derive(Debug, Clone)]
pub enum Plugin {
    WASM(Vec<u8>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RequestHeader {
    pub method: String,
    pub uri: String,
    pub version: String,
    pub header_map: HashMap<String, Vec<Vec<u8>>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResponseHeader {
    pub status_code: u16,
    pub version: String,
    pub header_map: HashMap<String, Vec<Vec<u8>>>,
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
    pub fn handle_request(&self, request: &mut Request<Body>) -> anyhow::Result<()> {
        // self.handle_raw(HandlerName::Request, header, origin_body);
        Ok(())
    }

    pub fn handle_response(&self, request: &mut Response<Body>) -> anyhow::Result<()> {
        // self.handle_raw(HandlerName::Response, header, origin_body);
        Ok(())
    }

    fn handle_raw(
        &self,
        hander_name: HandlerName,
        header: &[u8],
        origin_body: Vec<u8>,
    ) -> anyhow::Result<Vec<u8>> {
        match self {
            Plugin::WASM(data) => Self::handle_wasm(hander_name, data, header, origin_body),
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

impl From<request::Parts> for RequestHeader {
    fn from(parts: request::Parts) -> Self {
        unimplemented!()
    }
}

impl From<response::Parts> for ResponseHeader {
    fn from(parts: response::Parts) -> Self {
        unimplemented!()
    }
}
