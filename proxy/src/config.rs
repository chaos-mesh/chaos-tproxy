use crate::handler::http::Config as HandlerConfig;
use crate::tproxy::tproxy::Config as TproxyConfig;
use serde_derive::Deserialize;

#[derive(Debug, Eq, PartialEq, Clone, Deserialize)]
pub struct Config {
    pub tproxy_config: TproxyConfig,
}
