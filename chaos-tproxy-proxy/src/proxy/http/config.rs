use rustls::{ClientConfig, ServerConfig};

use crate::handler::http::rule::Rule;

#[derive(Clone)]
pub struct Config {
    pub http_config: HTTPConfig,
    pub tls_config: Option<TLSConfig>,
}

#[derive(Clone, Debug)]
pub struct HTTPConfig {
    pub proxy_port: u16,
    pub rules: Vec<Rule>,
}

#[derive(Clone)]
pub struct TLSConfig {
    pub tls_client_config: ClientConfig,
    pub tls_server_config: ServerConfig,
}
