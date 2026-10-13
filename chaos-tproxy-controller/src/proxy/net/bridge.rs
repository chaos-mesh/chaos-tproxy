use std::process::Command;

use anyhow::{anyhow, Context, Result};
use default_net::{self, Gateway};
use pnet::datalink::NetworkInterface;
use pnet::ipnetwork::{IpNetwork, Ipv4Network};
use rtnetlink::packet::route::Nla;
use rtnetlink::packet::RouteMessage;
use rtnetlink::Handle;
use uuid::Uuid;

use crate::proxy::net::iptables::clear_ebtables;
use crate::proxy::net::routes::{del_routes_noblock, get_routes_noblock, load_routes};

#[derive(Debug, Clone)]
pub struct NetEnv {
    pub netns: String,
    pub device: String,
    pub ip: String,

    bridge1: String,
    bridge2: String,

    veth1: String,
    veth2: String,
    veth3: String,
    pub veth4: String,

    save_routes: Vec<RouteMessage>,
}

impl NetEnv {
    pub async fn new(handle: &Handle) -> Self {
        let interfaces = pnet::datalink::interfaces();
        let prefix = loop {
            let key = Uuid::new_v4().to_string()[0..13].to_string();
            // For avoid there are any interface named start with key.
            if interfaces
                .iter()
                .all(|i| !i.name.as_str().starts_with(&key))
            {
                break key;
            }
        };
        let device = get_default_interface().unwrap();
        let netns = prefix.clone() + "ns";
        let bridge1 = prefix.clone() + "b1";
        let veth1 = prefix.clone() + "v1";
        let veth2 = "veth0".to_string();
        let bridge2 = prefix.clone() + "b2";
        let veth3 = "veth1".to_string();
        let veth4 = prefix + "v4";
        let ip = get_ipv4(&device).unwrap();

        let mut routes = get_routes_noblock(handle).await.unwrap();

        routes.reverse();

        Self {
            netns,
            device: device.name,
            ip,
            bridge1,
            bridge2,
            veth1,
            veth2,
            veth3,
            veth4,
            save_routes: routes,
        }
    }

    pub async fn setenv_bridge(&self, handle: &mut Handle) -> Result<()> {
        let Gateway {
            mac_addr: gateway_mac,
            ip_addr: gateway_ip,
        } = try_get_default_gateway()?;

        let gateway_ip_s = gateway_ip.to_string();
        let gateway_mac_s = gateway_mac.to_string();

        let save_dns = "cp /etc/resolv.conf /etc/resolv.conf.bak";
        let net: Ipv4Network = self
            .ip
            .parse()
            .context(format!("ip {} parsed error", self.ip))?;
        let net_ip32 = net.ip().to_string() + "/32";
        let rp_filter_br2 = format!("net.ipv4.conf.{}.rp_filter=0", self.bridge2);
        let rp_filter_v2 = format!("net.ipv4.conf.{}.rp_filter=0", self.veth2);
        let rp_filter_v3 = format!("net.ipv4.conf.{}.rp_filter=0", self.veth3);
        let arp_filter_br1 = format!("net.ipv4.conf.{}.arp_filter=1", self.bridge1);
        let arp_filter_v1 = format!("net.ipv4.conf.{}.arp_filter=1", self.veth1);
        let arp_filter_dev = format!("net.ipv4.conf.{}.arp_filter=1", self.device);
        let cmdvv = vec![
            bash_c(save_dns),
            ip_netns_add(&self.netns),
            ip_link_add_bridge(&self.bridge1),
            ip_link_add_veth_peer(&self.veth1, None, &self.veth2, Some(&self.netns)),
            ip_netns(&self.netns, ip_link_add_bridge(&self.bridge2)),
            ip_link_add_veth_peer(&self.veth4, None, &self.veth3, Some(&self.netns)),
            ip_link_set_up(&self.bridge1),
            vec!["sysctl", "-w", &arp_filter_br1],
            vec!["sysctl", "-w", &arp_filter_v1],
            vec!["sysctl", "-w", &arp_filter_dev],
            ip_link_set_up(&self.veth1),
            ip_netns(&self.netns, ip_link_set_up(&self.veth2)),
            ip_netns(&self.netns, ip_link_set_up(&self.bridge2)),
            ip_netns(&self.netns, ip_link_set_up(&self.veth3)),
            ip_link_set_up(&self.veth4),
            ip_link_set_master(&self.device, &self.bridge1),
            ip_link_set_master(&self.veth1, &self.bridge1),
            ip_netns(&self.netns, ip_link_set_master(&self.veth2, &self.bridge2)),
            ip_netns(&self.netns, ip_link_set_master(&self.veth3, &self.bridge2)),
            ip_netns(&self.netns, ip_link_set_up("lo")),
        ];
        execute_all(cmdvv)?;

        execute_all_with_log_error(vec![ip_address("del", &self.ip, &self.device)])?;

        let cmdvv = vec![
            ip_address("add", &self.ip, &self.veth4),
            arp_set(&gateway_ip_s, &gateway_mac_s, &self.veth1),
            arp_set(&gateway_ip_s, &gateway_mac_s, &self.veth4),
            ip_netns(
                &self.netns,
                arp_set(&gateway_ip_s, &gateway_mac_s, &self.veth2),
            ),
            ip_netns(
                &self.netns,
                arp_set(&gateway_ip_s, &gateway_mac_s, &self.bridge2),
            ),
            ip_route_add("default", &gateway_ip_s, &self.veth4),
            ip_netns(
                &self.netns,
                ip_route_add("default", &gateway_ip_s, &self.bridge2),
            ),
            ip_netns(
                &self.netns,
                vec![
                    "ip",
                    "route",
                    "add",
                    &net_ip32,
                    "dev",
                    &self.bridge2,
                    "proto",
                    "kernel",
                ],
            ),
            ip_netns(&self.netns, vec!["sysctl", "-w", "net.ipv4.ip_forward=1"]),
            ip_netns(
                &self.netns,
                vec!["sysctl", "-w", "net.ipv4.ip_nonlocal_bind=1"],
            ),
            ip_netns(&self.netns, vec!["sysctl", "-w", &rp_filter_br2]),
            ip_netns(&self.netns, vec!["sysctl", "-w", &rp_filter_v2]),
            ip_netns(&self.netns, vec!["sysctl", "-w", &rp_filter_v3]),
            ip_netns(
                &self.netns,
                vec!["sysctl", "-w", "net.ipv4.conf.lo.rp_filter=0"],
            ),
            ip_netns(
                &self.netns,
                vec!["sysctl", "-w", "net.ipv4.conf.all.rp_filter=0"],
            ),
            ip_netns(
                &self.netns,
                vec!["ip", "rule", "add", "fwmark", "1", "lookup", "100"],
            ),
            ip_netns(
                &self.netns,
                vec![
                    "ip",
                    "route",
                    "add",
                    "local",
                    "0.0.0.0/0",
                    "dev",
                    "lo",
                    "table",
                    "100",
                ],
            ),
        ];
        execute_all(cmdvv)?;
        let interfaces = pnet::datalink::interfaces();
        let veth4_mac = interfaces
            .iter()
            .find(|p| p.name == self.veth4)
            .context(format!("interface {} not found", self.veth4.clone()))?
            .mac
            .context(format!("mac {} not found", self.veth4.clone()))?
            .to_string();
        execute_all(vec![ip_netns(
            &self.netns,
            arp_set(&net.ip().to_string(), &veth4_mac, &self.bridge2),
        )])?;

        let all_routes = get_routes_noblock(handle).await?;

        let kernel_routes: Vec<RouteMessage> = all_routes
            .into_iter()
            .filter(|msg| {
                msg.header.table != 255
                    && msg.nlas.iter().any(|n| match n {
                        Nla::PrefSource(addr) => {
                            let digits: Vec<String> = addr
                                .clone()
                                .into_iter()
                                .map(|digit| digit.to_string())
                                .collect();
                            let addr_string = digits.join(".");
                            self.ip.contains(&addr_string)
                        }
                        _ => false,
                    })
                    && msg.nlas.iter().all(|n| !matches!(n, Nla::Gateway(_)))
            })
            .collect();
        del_routes_noblock(handle, kernel_routes).await?;
        Ok(())
    }

    pub async fn clear_bridge(&self, handle: &mut Handle) -> Result<()> {
        let restore_dns = "cp /etc/resolv.conf.bak /etc/resolv.conf";

        let cmdvv = vec![
            ip_netns_del(&self.netns),
            ip_link_del_bridge(&self.bridge1),
            ip_address("add", &self.ip, &self.device),
            bash_c(restore_dns),
            clear_ebtables(),
        ];
        execute_all_with_log_error(cmdvv)?;

        let routes = get_routes_noblock(handle).await.unwrap_or_else(|e| {
            tracing::error!("clear routes get_routes_noblock with error {}", e);
            vec![]
        });

        del_routes_noblock(handle, routes)
            .await
            .unwrap_or_else(|e| {
                tracing::error!("clear routes del_routes_noblock with error {}", e);
            });

        load_routes(handle, self.save_routes.clone())
            .await
            .unwrap_or_else(|e| {
                tracing::error!("clear routes load_routes with error {}", e);
            });

        let Gateway {
            mac_addr: gateway_mac,
            ip_addr: gateway_ip,
        } = try_get_default_gateway()?;

        if gateway_mac.octets().iter().all(|&i| i == 0) {
            return Ok(());
        }

        let gateway_ip = gateway_ip.to_string();
        let gateway_mac = gateway_mac.to_string();

        let cmdvv = vec![arp_set(&gateway_ip, &gateway_mac, self.device.as_str())];
        execute_all_with_log_error(cmdvv)?;
        Ok(())
    }
}

pub fn arp_set<'a>(ip: &'a str, mac: &'a str, device: &'a str) -> Vec<&'a str> {
    vec!["arp", "-s", ip, mac, "-i", device]
}

pub fn ip_netns_add(name: &str) -> Vec<&str> {
    vec!["ip", "netns", "add", name]
}

pub fn ip_netns_del(name: &str) -> Vec<&str> {
    vec!["ip", "netns", "delete", name]
}

pub fn ip_link_add_bridge(name: &str) -> Vec<&str> {
    vec!["ip", "link", "add", "name", name, "type", "bridge"]
}

pub fn bash_c(cmd: &str) -> Vec<&str> {
    vec!["sh", "-c", cmd]
}

pub fn ip_link_del_bridge(name: &str) -> Vec<&str> {
    vec!["ip", "link", "delete", "dev", name, "type", "bridge"]
}

pub fn ip_link_add_veth_peer<'a>(
    name1: &'a str,
    netns1: Option<&'a str>,
    name2: &'a str,
    netns2: Option<&'a str>,
) -> Vec<&'a str> {
    //ip link add p1 type veth peer p2 netns proxyns
    let mut cmd = vec!["ip", "link", "add", name1];
    if let Some(netns) = netns1 {
        cmd.extend_from_slice(&["netns", netns]);
    }
    cmd.extend_from_slice(&["type", "veth", "peer", name2]);
    if let Some(netns) = netns2 {
        cmd.extend_from_slice(&["netns", netns]);
    }
    cmd
}

pub fn ip_netns<'a>(netns: &'a str, cmdv: Vec<&'a str>) -> Vec<&'a str> {
    let mut cmd = vec!["ip", "netns", "exec", netns];
    cmd.extend_from_slice(cmdv.as_slice());
    cmd
}

pub fn ip_link_set_up(name: &str) -> Vec<&str> {
    vec!["ip", "link", "set", name, "up"]
}

pub fn ip_link_set_master<'a>(name: &'a str, master: &'a str) -> Vec<&'a str> {
    vec!["ip", "link", "set", name, "master", master]
}

pub fn os_err(stderr: Vec<u8>) -> Result<()> {
    if !stderr.is_empty() {
        tracing::debug!("stderr : {}", String::from_utf8_lossy(&stderr));
        return Err(anyhow::anyhow!(
            "stderr : {}",
            String::from_utf8_lossy(&stderr)
        ));
    };
    Ok(())
}

pub fn ip_address<'a>(action: &'a str, address: &'a str, device: &'a str) -> Vec<&'a str> {
    vec!["ip", "address", action, address, "dev", device]
}

pub fn ip_route_add<'a>(target: &'a str, gateway_ip: &'a str, device: &'a str) -> Vec<&'a str> {
    vec![
        "ip", "route", "add", target, "via", gateway_ip, "dev", device, "proto", "kernel", "onlink",
    ]
}

pub fn try_get_default_gateway() -> Result<Gateway> {
    let mut count = 5;
    while count > 0 {
        match default_net::get_default_gateway() {
            Ok(gateway) => return Ok(gateway),
            Err(e) => tracing::error!("{}", e),
        }
        count -= 1;
    }
    Err(anyhow!("tried 5 times but not get gateway"))
}

pub fn get_ipv4(device: &NetworkInterface) -> Option<String> {
    for ip in &device.ips {
        if let IpNetwork::V4(ipv4) = ip {
            return Some(ipv4.ip().to_string() + "/" + &ipv4.prefix().to_string());
        }
    }
    None
}

pub fn execute_all_with_log_error(cmdvv: Vec<Vec<&str>>) -> Result<()> {
    for cmdv in cmdvv {
        let _ = execute(cmdv);
    }
    Ok(())
}

pub fn execute_all(cmdvv: Vec<Vec<&str>>) -> Result<()> {
    for cmdv in cmdvv {
        execute(cmdv)?;
    }
    Ok(())
}

pub fn execute(cmdv: Vec<&str>) -> Result<()> {
    tracing::trace!("{:?}", cmdv);
    let mut iter = cmdv.iter();
    let mut cmd = match iter.next() {
        None => {
            return Ok(());
        }
        Some(s) => Command::new(s),
    };
    for s in iter {
        cmd.arg(*s);
    }
    let out = cmd
        .output()
        .context(format!("cmd output meet error : {:?}", cmdv))?;
    if !out.stdout.is_empty() {
        tracing::debug!("stdout : {}", String::from_utf8_lossy(&out.stdout));
    }
    os_err(out.stderr)
}

pub fn get_interface(name: String) -> Result<NetworkInterface> {
    let interfaces = pnet::datalink::interfaces();
    for interface in interfaces {
        if interface.name == name {
            return Ok(interface);
        }
    }
    Err(anyhow!("no valid interface"))
}

pub fn get_default_interface() -> Result<NetworkInterface> {
    let default_interface = default_net::get_default_interface().map_err(|e| anyhow!(e))?;
    select_default_interface(pnet::datalink::interfaces(), default_interface.index)
}

fn select_default_interface(
    interfaces: Vec<NetworkInterface>,
    default_index: u32,
) -> Result<NetworkInterface> {
    for interface in interfaces {
        if interface.index == default_index
            && !interface.is_loopback()
            && interface.is_up()
            && interface
                .ips
                .iter()
                .any(|ip| matches!(ip, IpNetwork::V4(_)))
        {
            return Ok(interface);
        }
    }
    Err(anyhow!("no valid interface"))
}

#[cfg(test)]
mod tests {
    use pnet::datalink::NetworkInterface;

    use super::select_default_interface;

    fn interface(index: u32, name: &str, ips: &[&str], flags: u32) -> NetworkInterface {
        NetworkInterface {
            name: name.into(),
            description: String::new(),
            index,
            mac: None,
            ips: ips.iter().map(|ip| ip.parse().unwrap()).collect(),
            flags,
        }
    }

    #[test]
    fn select_interface_skips_ipv6_only_interface() {
        let up = libc::IFF_UP as u32;
        let selected = select_default_interface(
            vec![
                interface(1, "eth0", &["fe80::1/64"], up),
                interface(2, "veth", &["192.0.2.4/24"], up),
            ],
            2,
        )
        .unwrap();
        assert_eq!(selected.name, "veth");
        assert_eq!(selected.ips, vec!["192.0.2.4/24".parse().unwrap()]);
    }

    #[test]
    fn select_default_interface_is_independent_of_inventory_order() {
        let up = libc::IFF_UP as u32;
        let secondary = interface(2, "secondary", &["198.51.100.2/24"], up);
        let original = interface(1, "eth0", &["fe80::1/64"], up);
        let active = interface(3, "veth", &["192.0.2.4/24"], up);
        for interfaces in [
            vec![secondary.clone(), original.clone(), active.clone()],
            vec![active.clone(), secondary, original],
        ] {
            let selected = select_default_interface(interfaces, 3).unwrap();
            assert_eq!(selected.name, "veth");
            assert_eq!(selected.index, 3);
            assert_eq!(selected.ips, vec!["192.0.2.4/24".parse().unwrap()]);
        }
    }

    #[test]
    fn select_interface_accepts_dual_stack_default_interface() {
        let selected = select_default_interface(
            vec![interface(
                1,
                "eth0",
                &["192.0.2.4/24", "fe80::1/64"],
                libc::IFF_UP as u32,
            )],
            1,
        )
        .unwrap();
        assert_eq!(selected.name, "eth0");
        assert_eq!(
            selected.ips,
            vec![
                "192.0.2.4/24".parse().unwrap(),
                "fe80::1/64".parse().unwrap()
            ]
        );
    }

    #[test]
    fn select_interface_does_not_fall_back_to_secondary_interface() {
        for invalid_default in [
            interface(1, "down", &["192.0.2.1/24"], 0),
            interface(
                1,
                "lo",
                &["127.0.0.1/8"],
                (libc::IFF_UP | libc::IFF_LOOPBACK) as u32,
            ),
            interface(1, "eth0", &["fe80::1/64"], libc::IFF_UP as u32),
        ] {
            let err = select_default_interface(
                vec![
                    invalid_default,
                    interface(2, "secondary", &["198.51.100.2/24"], libc::IFF_UP as u32),
                ],
                1,
            )
            .unwrap_err();
            assert_eq!(err.to_string(), "no valid interface");
        }
    }

    #[test]
    fn select_interface_rejects_missing_default_interface() {
        let err = select_default_interface(
            vec![interface(
                2,
                "secondary",
                &["198.51.100.2/24"],
                libc::IFF_UP as u32,
            )],
            1,
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "no valid interface");
    }
}
