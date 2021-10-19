use std::env;
use std::path::PathBuf;
use std::process::{ExitStatus, Stdio};

use rs_tproxy_proxy::raw_config::RawConfig as ProxyRawConfig;
use rs_tproxy_proxy::task::Task;
use tokio::process::Command;
use tracing::instrument;
use uuid::Uuid;

use super::controller::send_config;
use super::net::bridge::NetEnv;
use super::net::set_net::set_net;

#[derive(Debug, Clone)]
pub struct ProxyOpt {
    pub ipc_path: PathBuf,
    pub verbose: u8,
}

impl ProxyOpt {
    pub fn new(ipc_path: PathBuf, verbose: u8) -> Self {
        Self { ipc_path, verbose }
    }
}

#[derive(Debug)]
pub struct Proxy {
    pub opt: ProxyOpt,
    pub net_env: NetEnv,
    pub proxy_ports: Option<String>,
    pub task: Option<Task<ExitStatus>>,
}

impl Proxy {
    pub fn new(verbose: u8) -> Self {
        let uds_path = env::temp_dir()
            .join(Uuid::new_v4().to_string())
            .with_extension("sock");

        let opt = ProxyOpt::new(uds_path, verbose);
        Self {
            opt,
            net_env: NetEnv::new(),
            proxy_ports: None,
            task: None,
        }
    }

    #[instrument(skip(self, config))]
    pub async fn start(&mut self, config: ProxyRawConfig) -> anyhow::Result<()> {
        tracing::info!("transferring proxy raw config {:?}", &config);
        let opt = self.opt.clone();
        let exe_path = match std::env::current_exe() {
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "failed to get current exe path,error : {:?}",
                    e
                ));
            }
            Ok(path) => path,
        };

        tracing::info!("network device name {}", self.net_env.device.clone());
        match config.interface {
            None => {}
            Some(ref interface) => {
                self.net_env.set_ip_with_interface_name(interface)?;
            }
        }
        set_net(
            &self.net_env,
            config.proxy_ports.as_ref(),
            config.listen_port,
            config.safe_mode,
        )?;

        let mut proxy = Command::new("ip");
        proxy
            .arg("netns")
            .arg("exec")
            .arg(&self.net_env.netns)
            .arg(exe_path)
            .arg(format!(
                "-{}",
                String::from_utf8(vec![b'v'; self.opt.verbose as usize])?
            ))
            .arg("--proxy")
            .arg(format!("--ipc-path={}", opt.ipc_path.to_string_lossy()));
        tracing::info!("starting proxy");
        let mut process = match proxy.stdin(Stdio::piped()).spawn() {
            Ok(process) => {
                tracing::info!("proxy is running");
                process
            }
            Err(e) => {
                return Err(anyhow::anyhow!("failed to start sub proxy: {}", e));
            }
        };

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        send_config(&self.opt.ipc_path, &config).await?;
        self.task = Some(Task::start(async move { Ok(process.wait().await?) }));
        self.proxy_ports = config.proxy_ports;
        Ok(())
    }

    pub async fn stop(&mut self) -> anyhow::Result<()> {
        let _ = self.net_env.clear_bridge();
        if let Some(task) = self.task.take() {
            if let Some(status) = task.stop().await? {
                status.exit_ok()?;
            }
        }
        Ok(())
    }

    pub async fn reload(&mut self, config: ProxyRawConfig) -> anyhow::Result<()> {
        if self.task.is_none() {
            self.start(config).await?;
        } else if self.proxy_ports == config.proxy_ports {
            send_config(&self.opt.ipc_path, &config).await?;
        } else {
            self.stop().await?;
            self.start(config).await?;
        }
        Ok(())
    }
}
