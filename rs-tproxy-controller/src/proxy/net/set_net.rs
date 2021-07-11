
use crate::proxy::net::bridge::{NetEnv, execute_all};
use crate::proxy::net::iptables::set_iptables;
use std::option::Option::Some;

#[cfg(target_os = "linux")]
pub fn set_net(net_env: &NetEnv,proxy_ports : Option<String>, listen_port : u16) -> anyhow::Result<()> {
    net_env.setenv_bridge()?;
    let port = listen_port.to_string();
    if let Some(ref proxy_ports) = proxy_ports {
        execute_all(set_iptables(&net_env,Some(proxy_ports),&port))?;
    } else {
        execute_all(set_iptables(&net_env,None,&port))?;
    }

    Ok(())
}


#[cfg(target_os = "windows")]
pub fn set_env(){}