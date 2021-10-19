use crate::handler::http::rule::Rule;

#[derive(Debug, Clone)]
pub struct Config {
    pub proxy_port: u16,
    pub plugin_path: String,
    pub rules: Vec<Rule>,
}
