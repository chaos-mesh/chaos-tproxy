use crate::handler::http::rule::Rule;

#[derive(Debug, Clone)]
pub struct Config {
    pub proxy_port: u16,
    pub rules: Vec<Rule>,
}
