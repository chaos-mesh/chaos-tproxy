use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use structopt::StructOpt;

use super::super::config::Config;

#[derive(Debug, StructOpt)]
#[structopt(name = "proxy", about = "The option of rs-proxy.")]
struct Opt {
    ///path of config file
    #[structopt(parse(from_os_str))]
    input: PathBuf,
}

pub fn get_config() -> Config {
    let opt: Opt = Opt::from_args();
    get_config_from_opt(opt.input)
}

pub fn get_config_from_opt(path_buf: PathBuf) -> Config {
    let mut file = File::open(path_buf.to_str().unwrap()).unwrap();
    let mut buffer = String::new();
    let _s = file.read_to_string(&mut buffer).unwrap();
    let content = &buffer[..];
    match path_buf.extension().unwrap().to_str().unwrap() {
        "json" => {
            return serde_json::from_str(content).unwrap();
        }
        "yaml" => {
            return serde_yaml::from_str(content).unwrap();
        }
        _ => panic!("invalid file extension"),
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use crate::config::Config;
    use crate::handler::http::{
        Action, Config as HandlerConfig, HeaderFieldVec, PacketTarget, Selector,
    };
    use crate::tproxy::tproxy::Config as TproxyConfig;
    #[test]
    fn test_serde_util() {
        let conf = Config {
            tproxy_config: TproxyConfig {
                port: 58080,
                mark: 255,
                handler_config: HandlerConfig {
                    packet: PacketTarget::Response,
                    selector: Selector {
                        path: Some(b"/rs-tproxy".to_vec()),
                        method: Some(b"GET".to_vec()),
                        code: Some(b"400".to_vec()),
                        header_fields: Some(vec![HeaderFieldVec {
                            field_name: b"aname".to_vec(),
                            field_value: b"avalue".to_vec(),
                        }]),
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
