use std::option::Option::Some;

use crate::proxy::net::bridge::{bash_c, execute, execute_all, NetEnv};
use crate::proxy::net::iptables::{set_iptables, set_iptables_safe};

#[cfg(target_os = "linux")]
pub async fn set_net(
    net_env: &NetEnv,
    proxy_ports: Option<String>,
    listen_port: u16,
    safe: bool,
) -> anyhow::Result<()> {
    net_env.setenv_bridge().await?;
    let port = listen_port.to_string();
    let restore_dns = "cp /etc/resolv.conf.bak /etc/resolv.conf";

    if let Some(ref proxy_ports) = proxy_ports {
        execute_all(set_iptables(net_env, Some(proxy_ports), &port))?;
    } else {
        execute_all(set_iptables(net_env, None, &port))?;
    }

    if safe {
        execute_all(set_iptables_safe(net_env))?;
    }
    let _ = execute(bash_c(restore_dns));
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn set_env() {}
