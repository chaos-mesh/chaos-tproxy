use rs_tproxy_proxy::raw_config::RawRule;
use serde::{Deserialize, Serialize};

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)] // To prevent typos.
pub struct RawConfig {
    pub proxy_ports: Option<Vec<u16>>,
    pub safe_mode: Option<bool>,
    pub interface: Option<String>,
    pub rules: Option<Vec<RawRule>>,

    // Useless options now. Keep these options for upward compatible.
    pub listen_port: Option<u16>,
    pub proxy_mark: Option<i32>,
    pub ignore_mark: Option<i32>,
    pub route_table: Option<u8>,
}
