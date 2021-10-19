use std::convert::TryInto;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use log::Level;
use structopt::StructOpt;
use tokio::fs::read_to_string;

use crate::proxy::config::Config;
use crate::raw_config::RawConfig;

//todo: name & about. (need discussion)
#[derive(Debug, StructOpt)]
#[structopt(name = "proxy", about = "The option of proxy.")]
pub struct Opt {
    /// path of config file, required if interactive and daemon mode is disabled
    #[structopt(name = "FILE", parse(from_os_str))]
    pub input: Option<PathBuf>,

    /// Allows applying json config by stdin/stdout
    #[structopt(short, long)]
    pub interactive: bool,

    // The number of occurrences of the `v/verbose` flag
    /// Verbose mode (-v, -vv, -vvv, etc.)
    #[structopt(short, long, parse(from_occurrences))]
    pub verbose: u8,

    /// Only run the sub proxy.
    #[structopt(long)]
    pub proxy: bool,

    /// ipc path for sub proxy.
    #[structopt(long)]
    pub ipc_path: Option<PathBuf>,
}

impl Opt {
    pub fn get_level(&self) -> Level {
        match self.verbose {
            0 => Level::Error,
            1 => Level::Info,
            2 => Level::Debug,
            _ => Level::Trace,
        }
    }

    pub fn from_args_checked() -> Result<Self> {
        Self::from_args_safe()?.checked()
    }

    fn checked(self) -> Result<Self> {
        if !self.interactive && !self.proxy && self.input.is_none() {
            return Err(anyhow!("config file is required when interactive mode and daemon mode is all disabled, use `-h | --help` for more details"));
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
