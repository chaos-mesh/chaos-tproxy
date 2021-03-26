use serde::{Deserialize, Serialize};

use crate::handler::http::Config as HandlerConfig;

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct Config {
    pub port: u16,
    pub mark: i32,
    pub handler_config: HandlerConfig,
}
