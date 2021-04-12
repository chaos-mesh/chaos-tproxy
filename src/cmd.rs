use std::convert::TryInto;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use config::RawConfig;
use structopt::StructOpt;
use tokio::fs::read_to_string;

use crate::tproxy::config::Config;

pub mod config;

#[derive(Debug, StructOpt)]
#[structopt(name = "proxy", about = "The option of rs-proxy.")]
struct Opt {
    ///path of config file
    #[structopt(parse(from_os_str))]
    input: Option<PathBuf>,
}

pub async fn get_config() -> Result<Config> {
    let opt: Opt = Opt::from_args();
    get_config_from_opt(opt.input).await
}

pub async fn get_config_from_opt(path: Option<PathBuf>) -> Result<Config> {
    match path {
        None => RawConfig::default(),
        Some(path_buf) => {
            let buffer = read_to_string(&path_buf).await?;
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

    use super::config::{
        RawConfig, RawRequestAction, RawRequestRule, RawRequestSelector, RawResponseAction,
        RawResponseRule, RawResponseSelector, RawRules,
    };
    #[test]
    fn test_serde_util() -> Result<()> {
        let conf = RawConfig {
            listen_port: Some(58080),
            proxy_ports: vec![80],
            proxy_mark: Some(255),
            ignore_mark: Some(255),
            route_table: Some(100),
            rules: Some(RawRules {
                request: Some(vec![RawRequestRule {
                    selector: RawRequestSelector {
                        port: None,
                        path: Some("/rs-tproxy".to_string()),
                        method: Some("GET".to_string()),
                        headers: Some(
                            [("aname", "avalue")]
                                .iter()
                                .map(|(k, v)| (k.to_string(), v.to_string()))
                                .collect(),
                        ),
                    },
                    action: RawRequestAction::Delay(Duration::from_secs(1)),
                }]),
                response: Some(vec![RawResponseRule {
                    selector: RawResponseSelector {
                        port: None,
                        path: Some("/rs-tproxy".to_string()),
                        method: Some("GET".to_string()),
                        code: Some(80),
                        request_headers: Some(
                            [("aname", "avalue")]
                                .iter()
                                .map(|(k, v)| (k.to_string(), v.to_string()))
                                .collect(),
                        ),
                        response_headers: Some(
                            [("server", "nginx")]
                                .iter()
                                .map(|(k, v)| (k.to_string(), v.to_string()))
                                .collect(),
                        ),
                    },
                    action: RawResponseAction::Delay(Duration::from_secs(1)),
                }]),
            }),
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
