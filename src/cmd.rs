use std::convert::TryInto;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use config::RawConfig;
use structopt::StructOpt;
use tokio::fs::read_to_string;
use tracing_subscriber::filter::LevelFilter;

use crate::tproxy::config::Config;

pub mod config;

#[derive(Debug, StructOpt)]
#[structopt(name = "proxy", about = "The option of rs-proxy.")]
pub struct Opt {
    /// Allows to apply config by stdin/stdout
    #[structopt(short, long)]
    pub interactive: bool,

    // The number of occurrences of the `v/verbose` flag
    /// Verbose mode (-v, -vv, -vvv, etc.)
    #[structopt(short, long, parse(from_occurrences))]
    pub verbose: u8,

    /// path of config file, required if interactive mode is disabled
    #[structopt(name = "FILE", parse(from_os_str))]
    pub input: Option<PathBuf>,
}

impl Opt {
    pub fn get_level_filter(&self) -> LevelFilter {
        match self.verbose {
            0 => LevelFilter::ERROR,
            1 => LevelFilter::INFO,
            2 => LevelFilter::DEBUG,
            _ => LevelFilter::TRACE,
        }
    }

    pub fn from_args_checked() -> Result<Self> {
        Self::from_args_safe()?.checked()
    }

    fn checked(self) -> Result<Self> {
        if !self.interactive && self.input.is_none() {
            return Err(anyhow!("config file is required when interactive mode is disabled, use `-h | --help` for more details"));
        }
        Ok(self)
    }
}

pub async fn get_config_from_opt(opt: &Opt) -> Result<Config> {
    match opt.input {
        None => RawConfig::default(),
        Some(ref path_buf) => {
            let buffer = read_to_string(path_buf).await?;
            match path_buf.extension().and_then(|ext| ext.to_str()) {
                Some("json") => serde_json::from_str(&buffer)?,
                Some("yaml") => serde_yaml::from_str(&buffer)?,
                _ => return Err(anyhow!("invalid file extension")),
            }
        }
    }
    .try_into()
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use anyhow::Result;

    use super::config::{RawActions, RawConfig, RawRule, RawSelector, RawTarget};
    #[test]
    fn test_serde_util() -> Result<()> {
        let conf = RawConfig {
            listen_port: Some(58080),
            proxy_ports: vec![80],
            proxy_mark: Some(255),
            ignore_mark: Some(255),
            route_table: Some(100),
            rules: Some(vec![
                RawRule {
                    target: RawTarget::Request,
                    selector: RawSelector {
                        port: None,
                        path: Some("/rs-tproxy".to_string()),
                        method: Some("GET".to_string()),
                        request_headers: Some(
                            [("aname", "avalue")]
                                .iter()
                                .map(|(k, v)| (k.to_string(), v.to_string()))
                                .collect(),
                        ),
                        code: None,
                        response_headers: None,
                    },
                    actions: RawActions {
                        abort: None,
                        delay: Some(Duration::from_secs(1)),
                        replace: None,
                        patch: None,
                    },
                },
                RawRule {
                    target: RawTarget::Response,
                    selector: RawSelector {
                        port: None,
                        path: Some("/rs-tproxy".to_string()),
                        method: Some("GET".to_string()),
                        request_headers: Some(
                            [("aname", "avalue")]
                                .iter()
                                .map(|(k, v)| (k.to_string(), v.to_string()))
                                .collect(),
                        ),
                        code: Some(80),
                        response_headers: Some(
                            [("server", "nginx")]
                                .iter()
                                .map(|(k, v)| (k.to_string(), v.to_string()))
                                .collect(),
                        ),
                    },
                    actions: RawActions {
                        abort: Some(true),
                        delay: Some(Duration::from_secs(1)),
                        replace: None,
                        patch: None,
                    },
                },
            ]),
        };
        let json = serde_json::to_string(&conf)?;
        println!("{}", json);
        let conf_json_out: RawConfig = serde_json::from_str(&json)?;
        assert_eq!(conf_json_out, conf);
        let yaml = serde_yaml::to_string(&conf)?;
        println!("{}", yaml);
        let conf_yaml_out: RawConfig = serde_yaml::from_str(&yaml)?;
        assert_eq!(conf_yaml_out, conf);
        Ok(())
    }
}
