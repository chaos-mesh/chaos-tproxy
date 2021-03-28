use crate::handler::Rules;

#[derive(Debug, Clone)]
pub struct Config {
    pub listen_port: u16, // select random port if zero
    pub proxy_ports: Vec<u16>,
    pub proxy_mark: i32,
    pub ignore_mark: i32,
    pub rules: Rules,
}
