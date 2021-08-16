use std::env;
use std::path::PathBuf;
use std::process::Stdio;

use anyhow::Error;
use rs_tproxy_proxy::raw_config::RawConfig as ProxyRawConfig;
use tokio::process::Command;
use tokio::select;
use tokio::sync::oneshot::{channel, Receiver, Sender};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::proxy::net::bridge::NetEnv;
use crate::proxy::net::set_net::set_net;
use crate::proxy::uds_server::UdsDataServer;

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
    pub sender: Option<Sender<()>>,
    pub rx: Option<Receiver<()>>,
    pub task: Option<JoinHandle<Result<(), Error>>>,
}

impl Proxy {
    pub fn new(verbose: u8) -> Self {
        let uds_path = env::temp_dir()
            .join(Uuid::new_v4().to_string())
            .with_extension("sock");

        let opt = ProxyOpt::new(uds_path, verbose);
        let (sender, rx) = channel();
        Self {
            opt,
            net_env: NetEnv::new(),
            sender: Some(sender),
            rx: Some(rx),
            task: None,
        }
    }

    pub async fn exec(&mut self, config: ProxyRawConfig) -> anyhow::Result<()> {
        tracing::trace!("transferring proxy raw config : {:?}", &config);
        let uds_server = UdsDataServer::new(config.clone(), self.opt.ipc_path.clone());
        let listener = uds_server.bind()?;

        let server = uds_server.clone();
        tokio::spawn(async move {
            let _ = server
                .listen(listener)
                .await
                .map_err(|e| tracing::error!("{:?}", e));
        });

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

        match config.interface {
            None => {}
            Some(interface) => {
                self.net_env.set_ip_with_interface_name(&interface)?;
            }
        }
        set_net(
            &self.net_env,
            config.proxy_ports,
            config.listen_port,
            config.safe_mode,
        )?;

        let mut proxy = Command::new("ip");
        (&mut proxy)
            .arg("netns")
            .arg("exec")
            .arg(&self.net_env.netns)
            .arg(exe_path.clone())
            .arg(format!(
                "-{}",
                String::from_utf8(vec![b'v'; self.opt.verbose as usize]).unwrap()
            ))
            .arg("--proxy")
            .arg(format!("--ipc-path={}", opt.ipc_path.to_str().unwrap()));

        let rx = self.rx.take().unwrap();
        self.task = Some(tokio::spawn(async move {
            let mut process = match proxy.stdin(Stdio::piped()).spawn() {
                Ok(process) => process,
                Err(e) => {
                    return Err(anyhow::anyhow!("failed to exec sub proxy : {:?}", e));
                }
            };
            select! {
                _ = process.wait() => {}
                _ = rx => {
                    tracing::trace!("killing sub process");
                    let id = process.id().unwrap() as i32;
                    unsafe {
                        libc::kill(id, libc::SIGINT);
                    }
                }
            };
            Ok(())
        }));
        Ok(())
    }

    pub async fn stop(&mut self) -> anyhow::Result<()> {
        if let Some(task) = self.task.take() {
            if let Some(sender) = self.sender.take() {
                let _ = sender.send(());
            };
            &self.net_env.clear_bridge();
            let _ = task.await?;
        }
        Ok(())
    }

    pub async fn reload(&mut self, config: ProxyRawConfig) -> anyhow::Result<()> {
        self.stop().await?;
        if self.task.is_none() {
            let mut new = Self::new(self.opt.verbose);
            self.opt = new.opt;
            self.sender = new.sender.take();
            self.rx = new.rx.take();
        }
        self.exec(config).await
    }
}
