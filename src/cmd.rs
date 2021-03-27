use std::path::PathBuf;

use anyhow::{anyhow, Result};
use structopt::StructOpt;
use tokio::fs::read_to_string;

use super::config::Config;

#[derive(Debug, StructOpt)]
#[structopt(name = "proxy", about = "The option of rs-proxy.")]
struct Opt {
    ///path of config file
    #[structopt(parse(from_os_str))]
    input: PathBuf,
}

pub async fn get_config() -> Result<Config> {
    let opt: Opt = Opt::from_args();
    get_config_from_opt(opt.input).await
}

pub async fn get_config_from_opt(path_buf: PathBuf) -> Result<Config> {
    let buffer = read_to_string(&path_buf).await?;
    match path_buf.extension().and_then(|ext| ext.to_str()) {
        Some("json") => Ok(serde_json::from_str(&buffer)?),
        Some("yaml") => Ok(serde_yaml::from_str(&buffer)?),
        _ => Err(anyhow!("invalid file extension")),
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use crate::config::Config;
    use crate::handler::{Action, Config as HandlerConfig, PacketTarget, Selector};
    use crate::tproxy::config::Config as TproxyConfig;
    #[test]
    fn test_serde_util() {
        let conf = Config {
            tproxy_config: TproxyConfig {
                port: 58080,
                mark: 255,
                handler_config: HandlerConfig {
                    packet: PacketTarget::Response,
                    selector: Selector {
                        path: Some("/rs-tproxy".to_owned()),
                        method: Some("GET".to_owned()),
                        code: Some(400),
                        header_fields: Some(
                            [("aname".to_owned(), "aname".to_owned())]
                                .iter()
                                .cloned()
                                .collect(),
                        ),
                    },
                    action: Action::Delay(Duration::MILLISECOND * 1000),
                },
            },
        };
        let json = serde_json::to_string(&conf).unwrap();
        println!("{}", json);
        let conf_json_out: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(conf_json_out, conf);
        let yaml = serde_yaml::to_string(&conf).unwrap();
        println!("{}", yaml);
        let conf_yaml_out: Config = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(conf_yaml_out, conf);
    }
}
