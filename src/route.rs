use std::io;
use std::process::Command;

use iptables::new;
use tracing::debug;

use crate::tproxy::config::Config;

const DIVERT: &str = "DIVERT";
const PREROUTING: &str = "PREROUTING";
const CHAOS_PROXY_PREROUTING: &str = "CHAOS_PROXY_PREROUTING";
const OUTPUT: &str = "OUTPUT";
const CHAOS_PROXY_OUTPUT: &str = "CHAOS_PROXY_OUTPUT";
const MANGLE: &str = "mangle";

// do nothing if config.proxy_ports is None
pub fn set_all_routes(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let iptables = new(false)?;
    let proxy_ports = match config.proxy_ports.as_ref() {
        Some(ports) => ports,
        None => return Ok(()),
    };

    iptables.new_chain(MANGLE, DIVERT)?;
    iptables.append(
        MANGLE,
        PREROUTING,
        &format!("-p tcp -m socket -j {}", DIVERT),
    )?;
    iptables.append(
        MANGLE,
        DIVERT,
        &format!("-j MARK --set-mark {}", config.proxy_mark),
    )?;
    iptables.append(MANGLE, DIVERT, "-j ACCEPT")?;

    iptables.new_chain(MANGLE, CHAOS_PROXY_PREROUTING)?;
    iptables.append(
        MANGLE,
        CHAOS_PROXY_PREROUTING,
        &format!("-j RETURN -m mark --mark {:#x}", config.ignore_mark),
    )?;
    iptables.append(
        MANGLE,
        CHAOS_PROXY_PREROUTING,
        &format!(
            "-p tcp -j TPROXY --on-port {} --tproxy-mark {}",
            config.listen_port, config.proxy_mark
        ),
    )?;
    iptables.append(
        MANGLE,
        PREROUTING,
        &format!(
            "-p tcp --match multiport --dport {} -j {}",
            proxy_ports, CHAOS_PROXY_PREROUTING
        ),
    )?;

    iptables.new_chain(MANGLE, CHAOS_PROXY_OUTPUT)?;
    iptables.append(
        MANGLE,
        CHAOS_PROXY_OUTPUT,
        &format!("-j RETURN -m mark --mark {:#x}", config.ignore_mark),
    )?;
    iptables.append(
        MANGLE,
        CHAOS_PROXY_OUTPUT,
        &format!("-p tcp -j MARK --set-mark {}", config.proxy_mark),
    )?;
    iptables.append(
        MANGLE,
        OUTPUT,
        &format!(
            "-p tcp --match multiport --sport {} -j {}",
            proxy_ports, CHAOS_PROXY_OUTPUT
        ),
    )?;

    let err = set_ip_rule(config.route_table, config.proxy_mark)?;
    if !err.is_empty() {
        debug!(
            "stderr in setting ip rule: {}",
            String::from_utf8_lossy(&err)
        );
    }

    let err = set_ip_route(config.route_table)?;
    if !err.is_empty() {
        debug!(
            "stderr in setting ip route: {}",
            String::from_utf8_lossy(&err)
        );
    }

    debug!("set routes");
    Ok(())
}

// do nothing if config.proxy_ports is None
pub fn clear_routes(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    if config.proxy_ports.is_none() {
        return Ok(());
    }

    clear_ip_rule(config.route_table, config.proxy_mark)?;
    clear_ip_route(config.route_table)?;

    let iptables = new(false).expect("fail to init iptables");
    iptables.flush_table(MANGLE)?;
    iptables.delete_chain(MANGLE, DIVERT)?;
    iptables.delete_chain(MANGLE, CHAOS_PROXY_PREROUTING)?;
    iptables.delete_chain(MANGLE, CHAOS_PROXY_OUTPUT)?;

    debug!("clear routes");
    Ok(())
}

fn set_ip_route(table: u8) -> io::Result<Vec<u8>> {
    let stderr = Command::new("ip")
        .args(&[
            "route",
            "add",
            "local",
            "0.0.0.0/0",
            "dev",
            "lo",
            "table",
            &format!("{}", table),
        ])
        .output()?
        .stderr;
    Ok(stderr)
}

fn clear_ip_route(table: u8) -> io::Result<Vec<u8>> {
    let stderr = Command::new("ip")
        .args(&[
            "route",
            "del",
            "local",
            "0.0.0.0/0",
            "dev",
            "lo",
            "table",
            &format!("{}", table),
        ])
        .output()?
        .stderr;
    Ok(stderr)
}

fn set_ip_rule(table: u8, proxy_mark: i32) -> io::Result<Vec<u8>> {
    let stderr = Command::new("ip")
        .args(&[
            "rule",
            "add",
            "fwmark",
            &format!("{}", proxy_mark),
            "table",
            &format!("{}", table),
        ])
        .output()?
        .stderr;
    Ok(stderr)
}

fn clear_ip_rule(table: u8, proxy_mark: i32) -> io::Result<Vec<u8>> {
    let stderr = Command::new("ip")
        .args(&[
            "rule",
            "del",
            "fwmark",
            &format!("{}", proxy_mark),
            "table",
            &format!("{}", table),
        ])
        .output()?
        .stderr;
    Ok(stderr)
}
