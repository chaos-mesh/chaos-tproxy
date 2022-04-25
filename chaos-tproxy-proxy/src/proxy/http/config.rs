use crate::handler::http::rule::Rule;
use crate::raw_config::Role;

#[derive(Debug, Clone)]
pub struct Config {
    pub proxy_port: u16,
    pub rules: Vec<Rule>,
    pub role: Option<Role>,
}
