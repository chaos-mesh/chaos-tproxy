use anyhow::{anyhow, Context};
use libarp::interfaces::Interface;
use rtnetlink::Handle;

use crate::proxy::net::arp::gratuitous_arp;
use crate::proxy::net::bridge::{bash_c, execute, execute_all, get_interface, NetEnv};
use crate::proxy::net::iptables::{set_iptables, set_iptables_safe};
use crate::proxy::net::ping::GatewayPing;

#[cfg(target_os = "linux")]
pub(crate) async fn set_net(
    handle: &mut Handle,
    net_env: &mut NetEnv,
    proxy_ports: Option<String>,
    listen_port: u16,
    safe: bool,
) -> anyhow::Result<GatewayPing> {
    net_env.setenv_bridge(handle).await?;
    let port = listen_port.to_string();
    let restore_dns = "cp /etc/resolv.conf.bak /etc/resolv.conf";
    let device_interface = get_interface(net_env.veth4.clone())?;
    let device_mac = device_interface
        .mac
        .context("proxy veth has no MAC address")?
        .to_string();

    let arp_interface = Interface::new_by_name(net_env.veth4.clone().as_str())
        .context("proxy veth not found for ARP")?;
    gratuitous_arp(
        arp_interface.clone(),
        arp_interface
            .get_ip()
            .context("proxy veth has no IPv4 address")?,
        arp_interface.get_mac(),
    );

    execute_all(set_iptables(&net_env.netns, proxy_ports.as_deref(), &port))?;
    if safe {
        execute_all(set_iptables_safe(&net_env.netns, &net_env.ip))?;
    }
    net_env.install_outer_nat(device_mac)?;
    let _ = execute(bash_c(restore_dns));

    let gateway = default_net::get_default_gateway().map_err(|e| anyhow!(e))?;
    GatewayPing::start(gateway.ip_addr).await
}

#[cfg(target_os = "windows")]
pub fn set_env() {}
