use serde_derive::{Deserialize, Serialize};

use crate::tproxy::tproxy::Config as TproxyConfig;

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct Config {
    pub tproxy_config: TproxyConfig,
}
