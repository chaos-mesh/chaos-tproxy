use std::process::Command;
use std::time::SystemTime;

use anyhow::{anyhow, Context, Result};
use default_net;
use pnet::datalink::NetworkInterface;
use pnet::ipnetwork::{IpNetwork, Ipv4Network};
use rand::distributions::Alphanumeric;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use uuid::Uuid;

use crate::proxy::net::iptables::clear_ebtables;

#[derive(Debug, Clone)]
pub struct NetEnv {
    pub netns: String,
    pub device: String,
    pub ip: String,

    ip_route_store: String,

    bridge1: String,
    bridge2: String,

    veth1: String,
    veth2: String,
    veth3: String,
    pub veth4: String,
}

impl NetEnv {
    pub fn new() -> Self {
        let interfaces = pnet::datalink::interfaces();
        let prefix = loop {
            let d = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("Duration since UNIX_EPOCH failed");
            let key: String = StdRng::seed_from_u64(d.as_secs())
                .sample_iter(&Alphanumeric)
                .take(13)
                .map(char::from)
                .collect();
            // For avoid there are any interface named start with key.
            if interfaces
                .iter()
                .all(|i| !i.name.as_str().starts_with(&key))
            {
                break key;
            }
        };
        let ip_route_store = "ip_route_store".to_string() + &Uuid::new_v4().to_string();
        let device = get_default_interface().unwrap();
        let netns = prefix.clone() + "ns";
        let bridge1 = prefix.clone() + "b1";
        let veth1 = prefix.clone() + "v1";
        let veth2 = "veth0".to_string();
        let bridge2 = prefix.clone() + "b2";
        let veth3 = "veth1".to_string();
        let veth4 = prefix + "v4";
        let ip = get_ipv4(&device).unwrap();
        Self {
            netns,
            device: device.name,
            ip,
            ip_route_store,
            bridge1,
            bridge2,
            veth1,
            veth2,
            veth3,
            veth4,
        }
    }

    pub fn set_ip_with_interface_name(&mut self, interface: &str) -> anyhow::Result<()> {
        for i in pnet::datalink::interfaces() {
            if i.name == interface {
                self.device = i.name.clone();
                self.ip = get_ipv4(&i).unwrap();
                return Ok(());
            }
        }
        return Err(anyhow!("interface : {} not found", interface));
    }

    pub fn setenv_bridge(&self) -> Result<()> {
        let gateway_ip = match try_get_default_gateway_ip() {
            Ok(s) => s,
            Err(e) => return Err(e),
        };
        let gateway_mac = match default_net::get_default_gateway_mac(gateway_ip.clone()) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("{}", e);
                return Err(anyhow!(e));
            }
        };
        let save = format!("ip route save table all > {}", &self.ip_route_store);
        let save_dns = "cp /etc/resolv.conf /etc/resolv.conf.bak";
        let net: Ipv4Network = self
            .ip
            .parse()
            .context(format!("ip {} parsed error", self.ip))?;
        let net_ip32 = net.ip().to_string() + "/32";
        let rp_filter_br2 = format!("net.ipv4.conf.{}.rp_filter=0", &self.bridge2);
        let rp_filter_v2 = format!("net.ipv4.conf.{}.rp_filter=0", &self.veth2);
        let rp_filter_v3 = format!("net.ipv4.conf.{}.rp_filter=0", &self.veth3);
        let cmdvv = vec![
            bash_c(&save),
            bash_c(save_dns),
            ip_netns_add(&self.netns),
            ip_link_add_bridge(&self.bridge1),
            ip_link_add_veth_peer(&self.veth1, None, &self.veth2, Some(&self.netns)),
            ip_netns(&self.netns, ip_link_add_bridge(&self.bridge2)),
            ip_link_add_veth_peer(&self.veth4, None, &self.veth3, Some(&self.netns)),
            ip_link_set_up(&self.bridge1),
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
            ip_address("del", &self.ip, &self.device),
            ip_address("add", &self.ip, &self.veth4),
            arp_set(&gateway_ip, &gateway_mac, &self.veth1),
            arp_set(&gateway_ip, &gateway_mac, &self.veth4),
            ip_netns(&self.netns, arp_set(&gateway_ip, &gateway_mac, &self.veth2)),
            ip_netns(
                &self.netns,
                arp_set(&gateway_ip, &gateway_mac, &self.bridge2),
            ),
            ip_route_add("default", &gateway_ip, &self.veth4),
            ip_netns(
                &self.netns,
                ip_route_add("default", &gateway_ip, &self.bridge2),
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
        let _ = execute(ip_netns(
            &self.netns,
            arp_set(&net.ip().to_string(), &veth4_mac, &self.bridge2),
        ))?;
        Ok(())
    }

    pub fn clear_bridge(&self) -> Result<()> {
        let restore_dns = "cp /etc/resolv.conf.bak /etc/resolv.conf";
        let remove_store = format!("rm -f {}", &self.ip_route_store);

        let flush_main_route = "ip route flush table main";

        let cmdvv = vec![
            ip_netns_del(&self.netns),
            ip_link_del_bridge(&self.bridge1),
            ip_address("add", &self.ip, &self.device),
            bash_c(restore_dns),
            bash_c(flush_main_route),
            clear_ebtables(),
        ];
        execute_all_with_log_error(cmdvv)?;

        let ip_routes = restore_all_ip_routes(&self.ip_route_store)?;
        let iproute_cmds: Vec<Vec<&str>> = ip_routes.iter().map(|s| bash_c(&**s)).collect();
        execute_all_with_log_error(iproute_cmds)?;

        let cmdvv = vec![bash_c(&remove_store)];
        execute_all_with_log_error(cmdvv)?;
        Ok(())
    }
}

impl Default for NetEnv {
    fn default() -> Self {
        Self::new()
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

pub fn try_get_default_gateway_ip() -> Result<String> {
    match system_gateway::gateway() {
        Ok(ip) => return Ok(ip),
        Err(e) => {
            tracing::error!("{}", e);
            let mut count = 5;
            while count > 0 {
                let gataway_ip = default_net::get_default_gateway_ip();
                match gataway_ip {
                    Ok(ip) => return Ok(ip),
                    Err(e) => tracing::error!("{}", e),
                }
                count -= 1;
            }
        }
    };
    Err(anyhow!("tried 5 times but icmp target 8.8.8.8"))
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
        let _ = execute(cmdv)?;
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
    let interfaces = pnet::datalink::interfaces();
    for interface in interfaces {
        if !interface.is_loopback() && interface.is_up() && !interface.ips.is_empty() {
            return Ok(interface);
        }
    }
    Err(anyhow!("no valid interface"))
}

pub fn restore_all_ip_routes(path: &str) -> Result<Vec<String>> {
    let cmd_string = format!("ip route showdump < {}", path);
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(cmd_string);
    let stdo = cmd.output()?.stdout;
    let out = String::from_utf8_lossy(stdo.as_slice());

    let mut ip_routes: Vec<_> = out.split('\n').collect();
    ip_routes.reverse();
    let mut route_cmds: Vec<String> = Vec::new();
    for ip_route in ip_routes {
        if !ip_route.is_empty() {
            route_cmds.push(format!("{} {}", "ip route add", ip_route));
        }
    }
    Ok(route_cmds)
}
