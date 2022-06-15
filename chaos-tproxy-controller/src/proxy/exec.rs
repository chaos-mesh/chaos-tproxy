use std::env;
use std::path::PathBuf;
use std::process::Stdio;

use anyhow::Error;
use chaos_tproxy_proxy::raw_config::RawConfig as ProxyRawConfig;
use rtnetlink::{new_connection, Handle};
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
    pub rtnl_handle: Handle,
    pub sender: Option<Sender<()>>,
    pub rx: Option<Receiver<()>>,
    pub task: Option<JoinHandle<Result<(), Error>>>,
}

impl Proxy {
    pub async fn new(verbose: u8) -> Self {
        let uds_path = env::temp_dir()
            .join(Uuid::new_v4().to_string())
            .with_extension("sock");

        let opt = ProxyOpt::new(uds_path, verbose);
        let (sender, rx) = channel();

        let (conn, handle, _) = new_connection().unwrap();
        tokio::spawn(conn);
        Self {
            opt,
            net_env: NetEnv::new(&handle).await,
            rtnl_handle: handle,
            sender: Some(sender),
            rx: Some(rx),
            task: None,
        }
    }

    pub async fn exec(&mut self, config: ProxyRawConfig) -> anyhow::Result<()> {
        tracing::info!("transferring proxy raw config {:?}", &config);
        let uds_server = UdsDataServer::new(config.clone(), self.opt.ipc_path.clone());
        let listener = uds_server.bind()?;

        let server = uds_server;
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

        tracing::info!("Network device name {}", self.net_env.device.clone());
        match config.interface {
            None => {}
            Some(interface) => {
                self.net_env.set_ip_with_interface_name(&interface)?;
            }
        }
        set_net(
            &mut self.rtnl_handle,
            &self.net_env,
            config.proxy_ports,
            config.listen_port,
            config.safe_mode,
        )
        .await?;

        let mut proxy = Command::new("ip");
        proxy
            .arg("netns")
            .arg("exec")
            .arg(&self.net_env.netns)
            .arg(exe_path)
            .arg(format!(
                "-{}",
                String::from_utf8(vec![b'v'; self.opt.verbose as usize]).unwrap()
            ))
            .arg("--proxy")
            .arg(format!("--ipc-path={}", opt.ipc_path.to_str().unwrap()));

        let rx = self.rx.take().unwrap();
        self.task = Some(tokio::spawn(async move {
            tracing::info!("Proxy executor Starting proxy.");
            let mut process = match proxy.stdin(Stdio::piped()).spawn() {
                Ok(process) => {
                    tracing::info!("Proxy executor Proxy is running.");
                    process
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("failed to exec sub proxy : {:?}", e));
                }
            };
            select! {
                _ = process.wait() => {}
                _ = rx => {
                    tracing::info!("Proxy executor killing sub process");
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
            let _ = self.net_env.clear_bridge(&mut self.rtnl_handle).await;
            let _ = task.await?;
        }
        Ok(())
    }

    pub async fn reload(&mut self, config: ProxyRawConfig) -> anyhow::Result<()> {
        self.stop().await?;
        if config.proxy_ports.is_none() {
            return Ok(());
        }
        if self.task.is_none() {
            let mut new = Self::new(self.opt.verbose).await;
            self.net_env = new.net_env;
            self.opt = new.opt;
            self.sender = new.sender.take();
            self.rx = new.rx.take();
        }

        match self.exec(config).await {
            Err(e) => {
                self.net_env.clear_bridge(&mut self.rtnl_handle).await?;
                Err(e)
            }
            Ok(_) => Ok(()),
        }
    }
}
