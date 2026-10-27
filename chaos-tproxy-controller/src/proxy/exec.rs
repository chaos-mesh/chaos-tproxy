use std::env;
use std::process::Stdio;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use chaos_tproxy_proxy::bootstrap::BootstrapServer;
use chaos_tproxy_proxy::raw_config::RawConfig as ProxyRawConfig;
use rtnetlink::{new_connection, Handle};
use tokio::process::{Child, Command};
use uuid::Uuid;

use crate::proxy::net::bridge::NetEnv;
use crate::proxy::net::ping::GatewayPing;
use crate::proxy::net::set_net::set_net;

const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const STOP_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug)]
pub struct Proxy {
    verbose: u8,
    rtnl_handle: Handle,
    run: Option<Run>,
}

#[derive(Debug)]
struct Run {
    net: NetEnv,
    child: Option<Child>,
    bootstrap: Option<BootstrapServer>,
    ping: Option<GatewayPing>,
}

impl Proxy {
    pub async fn new(verbose: u8) -> Self {
        let (conn, handle, _) = new_connection().unwrap();
        tokio::spawn(conn);
        Self {
            verbose,
            rtnl_handle: handle,
            run: None,
        }
    }

    pub async fn stop(&mut self) -> Result<()> {
        if let Some(run) = self.run.as_mut() {
            run.stop(&mut self.rtnl_handle).await?;
            self.run = None;
        }
        Ok(())
    }

    pub async fn reload(&mut self, config: ProxyRawConfig) -> Result<()> {
        self.stop().await?;
        if config.proxy_ports.is_none() {
            return Ok(());
        }
        self.run = Some(Run {
            net: NetEnv::new()?,
            child: None,
            bootstrap: None,
            ping: None,
        });
        let result = self
            .run
            .as_mut()
            .unwrap()
            .start(&mut self.rtnl_handle, config, self.verbose)
            .await;
        if let Err(error) = result {
            return match self.stop().await {
                Ok(()) => Err(error),
                Err(cleanup) => Err(error.context(format!("startup rollback failed: {cleanup:#}"))),
            };
        }
        Ok(())
    }
}

impl Run {
    async fn start(
        &mut self,
        handle: &mut Handle,
        config: ProxyRawConfig,
        verbose: u8,
    ) -> Result<()> {
        let exe_path = env::current_exe().context("locate proxy executable")?;
        let ipc_path = env::temp_dir()
            .join(Uuid::new_v4().to_string())
            .with_extension("sock");
        self.bootstrap = Some(BootstrapServer::bind(ipc_path.clone(), &config)?);
        self.ping = Some(
            set_net(
                handle,
                &mut self.net,
                config.proxy_ports,
                config.listen_port,
                config.safe_mode,
            )
            .await?,
        );

        let mut command = Command::new("ip");
        command
            .args(["netns", "exec"])
            .arg(&self.net.netns)
            .arg(exe_path);
        if verbose > 0 {
            command.arg(format!("-{}", "v".repeat(verbose as usize)));
        }
        command.arg("--proxy").arg("--ipc-path").arg(ipc_path);
        self.child = Some(
            command
                .stdin(Stdio::null())
                .kill_on_drop(true)
                .spawn()
                .context("spawn proxy child")?,
        );
        wait_for_startup(
            self.child.as_mut().unwrap(),
            self.bootstrap.as_ref().unwrap(),
            STARTUP_TIMEOUT,
        )
        .await?;
        self.bootstrap.as_ref().unwrap().clear()?;
        self.bootstrap = None;
        Ok(())
    }

    async fn stop(&mut self, handle: &mut Handle) -> Result<()> {
        let mut errors = Vec::new();
        if let Some(ping) = self.ping.as_mut() {
            if let Err(error) = ping.stop().await {
                errors.push(format!("stop gateway ping: {error:#}"));
            }
            self.ping = None;
        }
        if let Some(child) = self.child.as_mut() {
            match stop_child(child).await {
                Ok(()) => self.child = None,
                Err(error) => errors.push(format!("stop proxy child: {error:#}")),
            }
        }
        if let Some(bootstrap) = self.bootstrap.as_ref() {
            match bootstrap.clear() {
                Ok(()) => self.bootstrap = None,
                Err(error) => errors.push(format!("{error:#}")),
            }
        }
        // A living child can keep its network namespace and veth peers alive.
        if self.child.is_none() {
            if let Err(error) = self.net.clear_bridge(handle).await {
                errors.push(format!("restore network: {error:#}"));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            bail!("{}", errors.join("; "))
        }
    }
}

async fn wait_for_startup(
    child: &mut Child,
    bootstrap: &BootstrapServer,
    timeout: Duration,
) -> Result<()> {
    tokio::select! {
        biased;
        result = bootstrap.wait_ready(timeout) => {
            result?;
            if let Some(status) = child.try_wait()? {
                bail!("proxy exited during startup: {}", status);
            }
            Ok(())
        }
        status = child.wait() => {
            bail!("proxy exited before readiness: {}", status?);
        }
    }
}

async fn stop_child(child: &mut Child) -> Result<()> {
    if child.try_wait()?.is_some() {
        return Ok(());
    }
    let pid = child.id().context("proxy child has no process id")?;
    if unsafe { libc::kill(pid as i32, libc::SIGINT) } != 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::ESRCH) {
            return Err(error).context("signal proxy child");
        }
    }
    match tokio::time::timeout(STOP_TIMEOUT, child.wait()).await {
        Ok(result) => {
            result?;
        }
        Err(_) => {
            child
                .start_kill()
                .context("kill unresponsive proxy child")?;
            tokio::time::timeout(STOP_TIMEOUT, child.wait())
                .await
                .context("timed out reaping proxy child")??;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn early_child_exit_rejects_startup() {
        let path = env::temp_dir().join(format!("{}.sock", Uuid::new_v4()));
        let bootstrap = BootstrapServer::bind(path, &ProxyRawConfig::default()).unwrap();
        let mut child = Command::new("sh").args(["-c", "exit 17"]).spawn().unwrap();
        let error = wait_for_startup(&mut child, &bootstrap, Duration::from_secs(1))
            .await
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "proxy exited before readiness: exit status: 17"
        );
        assert_eq!(child.try_wait().unwrap().unwrap().code(), Some(17));
        bootstrap.clear().unwrap();
    }

    #[tokio::test]
    async fn unready_child_is_terminated_and_reaped() {
        let path = env::temp_dir().join(format!("{}.sock", Uuid::new_v4()));
        let bootstrap = BootstrapServer::bind(path, &ProxyRawConfig::default()).unwrap();
        let mut child = Command::new("sleep")
            .arg("30")
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let error = wait_for_startup(&mut child, &bootstrap, Duration::from_millis(10))
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), "timed out waiting for proxy readiness");
        stop_child(&mut child).await.unwrap();
        bootstrap.clear().unwrap();
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(
            child.try_wait().unwrap().unwrap().signal(),
            Some(libc::SIGINT)
        );
        stop_child(&mut child).await.unwrap();
    }
}
