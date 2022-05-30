use std::option::Option::Some;

use rtnetlink::Handle;

use crate::proxy::net::bridge::{bash_c, execute, execute_all, get_interface, NetEnv};
use crate::proxy::net::iptables::{set_iptables, set_iptables_safe};

#[cfg(target_os = "linux")]
pub async fn set_net(
    handle: &mut Handle,
    net_env: &NetEnv,
    proxy_ports: Option<String>,
    listen_port: u16,
    safe: bool,
) -> anyhow::Result<()> {
    net_env.setenv_bridge(handle).await?;
    let port = listen_port.to_string();
    let restore_dns = "cp /etc/resolv.conf.bak /etc/resolv.conf";
    let device_interface = get_interface(net_env.veth4.clone()).unwrap();
    let device_mac = device_interface.mac.unwrap().to_string();

    if let Some(ref proxy_ports) = proxy_ports {
        execute_all(set_iptables(net_env, Some(proxy_ports), &port, &device_mac))?;
    } else {
        execute_all(set_iptables(net_env, None, &port, &device_mac))?;
    }

    if safe {
        execute_all(set_iptables_safe(net_env, &device_mac))?;
    }
    let _ = execute(bash_c(restore_dns));
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn set_env() {}
