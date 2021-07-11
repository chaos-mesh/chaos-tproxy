use anyhow::{Result, anyhow};
use pnet::datalink::NetworkInterface;
use pnet::ipnetwork::IpNetwork;
use std::process::Command;
use uuid::Uuid;


#[derive(Debug, Clone)]
pub struct NetEnv {
    pub netns : String,
    device : String,
    ip : String,

    bridge1: String,
    bridge2: String,

    veth1 : String,
    veth2 : String,
    veth3 : String,
    veth4 : String,
}

impl NetEnv {
    pub fn new() -> Self {
        //todo
        let netns = Uuid::new_v4().to_string()[0..15].to_string();
        let bridge1 = Uuid::new_v4().to_string()[0..15].to_string();
        let veth1 = Uuid::new_v4().to_string()[0..15].to_string();
        let veth2 = Uuid::new_v4().to_string()[0..15].to_string();
        let bridge2 = Uuid::new_v4().to_string()[0..15].to_string();
        let veth3 = Uuid::new_v4().to_string()[0..15].to_string();
        let veth4 = Uuid::new_v4().to_string()[0..15].to_string();
        let device = get_default_interface().unwrap();
        let ip = get_ipv4(&device).unwrap();
        Self {
            netns,
            device: device.name,
            ip,
            bridge1,
            bridge2,
            veth1,
            veth2,
            veth3,
            veth4
        }
    }

    pub fn setenv_bridge(&self) -> Result<()> {
        let cmdvv = vec![
            ip_netns_add(&self.netns),
            ip_link_add_bridge(&self.bridge1),
            ip_link_add_veth_peer(&self.veth1, None, &self.veth2, Some(&self.netns)),
            ip_netns(&self.netns,ip_link_add_bridge(&self.bridge2)),
            ip_link_add_veth_peer(&self.veth3, Some(&self.netns), &self.veth4, None),
            ip_link_set_up(&self.bridge1),
            ip_link_set_up(&self.veth1),
            ip_netns(&self.netns,ip_link_set_up(&self.veth2)),
            ip_netns(&self.netns,ip_link_set_up(&self.bridge2)),
            ip_netns(&self.netns,ip_link_set_up(&self.veth3)),
            ip_link_set_up(&self.veth4),
            ip_link_set_master(&self.device, &self.bridge1),
            ip_link_set_master(&self.veth1, &self.bridge1),
            ip_netns(&self.netns,ip_link_set_master(&self.veth2, &self.bridge2)),
            ip_netns(&self.netns,ip_link_set_master(&self.veth3, &self.bridge2)),
            ip_netns(&self.netns,ip_link_set_up("lo")),
            ip_address("del",&self.ip,&self.device),
            ip_address("add",&self.ip,&self.veth4),
            ip_route_add_default(&self.veth4),
            ip_netns(&self.netns, ip_route_add_default(&self.bridge2)),
            ip_netns(&self.netns, vec!["sysctl","-w","net.ipv4.ip_forward=1"]),
            ip_netns(&self.netns, vec!["sysctl","-w","net.ipv4.ip_nonlocal_bind=1"]),
            ip_netns(&self.netns, vec!["ip", "rule", "add", "fwmark", "1", "lookup", "100"]),
            ip_netns(&self.netns, vec!["ip", "route", "add", "local", "0.0.0.0/0", "dev", "lo", "table", "100"]),
        ];
        execute_all(cmdvv)?;
        Ok(())
    }

    pub fn clear_bridge(&self) -> Result<()> {
        let cmdvv = vec![
            ip_netns_del(&self.netns),
            ip_link_del_bridge(&self.bridge1),
            ip_address("add",&self.ip,&self.device),
        ];
        execute_all(cmdvv)?;
        Ok(())
    }
}


pub fn ip_netns_add(name : &str) -> Vec<&str> {
    vec!["ip", "netns", "add", name]
}

pub fn ip_netns_del(name : &str) -> Vec<&str> {
    vec!["ip", "netns", "delete", name]
}

pub fn ip_link_add_bridge(name: &str) -> Vec<&str> {
    vec!["ip", "link", "add", "name", name, "type", "bridge"]
}

pub fn ip_link_del_bridge(name: &str) -> Vec<&str> {
    vec!["ip", "link", "delete", "name", name, "type", "bridge"]
}

pub fn ip_link_add_veth_peer<'a>(name1: &'a str, netns1: Option<&'a str>, name2: &'a str, netns2: Option<&'a str>) -> Vec<&'a str> {
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

pub fn ip_netns<'a>(netns:&'a str,cmdv : Vec<&'a str>) -> Vec<&'a str> {
    let mut cmd = vec!["ip", "netns","exec",netns];
    cmd.extend_from_slice(cmdv.as_slice());
    cmd
}

pub fn ip_link_set_up(name : &str) -> Vec<&str> {
    vec!["ip","link","set",name,"up"]
}

pub fn ip_link_set_master<'a>(name : &'a str, master: &'a str) -> Vec<&'a str> {
    vec!["ip","link","set",name,"master",master]
}

pub fn os_err(stderr : Vec<u8>) -> Result<()> {
    if !stderr.is_empty() {
        tracing::debug!(
            "stderr : {}",
            String::from_utf8_lossy(&stderr)
        );
        return Err(anyhow::anyhow!("stderr : {}",String::from_utf8_lossy(&stderr)))
    };
    Ok(())
}

pub fn ip_address<'a>(action: &'a str, address: &'a str, device:&'a str) -> Vec<&'a str> {
    vec!["ip","address",action, address,"dev",device]
}

pub fn ip_route_add_default(device: &str) -> Vec<&str> {
    vec!["ip", "route", "add", "default", "dev", device, "proto", "kernel", "scope", "link"]
}

pub fn get_ipv4(device: &NetworkInterface) ->Option<String> {
    for ip in &device.ips {
        match ip {
            IpNetwork::V4(ipv4) => {
                return Some(ipv4.ip().to_string() + "/" + &ipv4.prefix().to_string());
            }
            _ => {}
        }
    };
    None
}

pub fn execute_all(cmdvv: Vec<Vec<&str>>) -> Result<()> {
    for cmdv in cmdvv {
        let _ = execute(cmdv);
    };
    Ok(())
}

pub fn execute(cmdv:Vec<&str>) -> Result<()> {
    tracing::debug!("{:?}",cmdv);
    let mut iter = cmdv.iter();
    let mut cmd = match iter.next() {
        None =>{return Ok(());}
        Some(s) => {
            Command::new(s)
        }
    };
    for s in iter {
        cmd.arg(*s);
    }
    os_err(cmd.output().unwrap().stderr)
}

pub fn get_default_interface() -> Result<NetworkInterface> {
    let interfaces = pnet::datalink::interfaces();
    for interface in interfaces {
        if !interface.is_loopback() && interface.is_up() && !interface.ips.is_empty() {
            return Ok(interface)
        }
    };
    Err(anyhow!("no valid interface"))
}

#[cfg(test)]
mod test {
    use crate::proxy::net::bridge::try_into;

    #[test]
    fn test_try_into() {
        let a = vec!["ls", "b", "c"];
        let cmd = try_into(a).unwrap();
        println!("{:?}", cmd);
    }
}
