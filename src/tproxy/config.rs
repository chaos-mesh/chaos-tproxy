use crate::handler::Rule;

#[derive(Debug, Clone)]
pub struct Config {
    pub listen_port: u16, // select random port if zero
    pub proxy_ports: String,
    pub proxy_mark: i32,
    pub ignore_mark: i32,
    pub route_table: u8,
    pub rules: Vec<Rule>,
}
