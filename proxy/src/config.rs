use crate::tproxy::tproxy::Config as TproxyConfig;
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct Config {
    pub tproxy_config: TproxyConfig,
}
