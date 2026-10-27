use crate::proxy::net::bridge::ip_netns;

pub fn set_iptables<'a>(
    netns: &'a str,
    proxy_ports: Option<&'a str>,
    listen_port: &'a str,
) -> Vec<Vec<&'a str>> {
    let cmdv = match proxy_ports {
        Some(proxy_ports) => ip_netns(
            netns,
            vec![
                "iptables",
                "-t",
                "mangle",
                "-A",
                "PREROUTING",
                "-p",
                "tcp",
                "-m",
                "multiport",
                "--dports",
                proxy_ports,
                "-j",
                "TPROXY",
                "--tproxy-mark",
                "0x1/0x1",
                "--on-port",
                listen_port,
            ],
        ),
        None => ip_netns(
            netns,
            vec![
                "iptables",
                "-t",
                "mangle",
                "-A",
                "PREROUTING",
                "-p",
                "tcp",
                "-j",
                "TPROXY",
                "--tproxy-mark",
                "0x1/0x1",
                "--on-port",
                listen_port,
            ],
        ),
    };

    vec![
        ip_netns(netns, vec!["iptables", "-t", "mangle", "-N", "DIVERT"]),
        ip_netns(
            netns,
            vec![
                "iptables",
                "-t",
                "mangle",
                "-A",
                "PREROUTING",
                "-p",
                "tcp",
                "-m",
                "socket",
                "-j",
                "DIVERT",
            ],
        ),
        ip_netns(
            netns,
            vec![
                "iptables",
                "-t",
                "mangle",
                "-A",
                "DIVERT",
                "-j",
                "MARK",
                "--set-mark",
                "1",
            ],
        ),
        ip_netns(
            netns,
            vec!["iptables", "-t", "mangle", "-A", "DIVERT", "-j", "ACCEPT"],
        ),
        cmdv,
        ip_netns(
            netns,
            vec![
                "ebtables-legacy",
                "-t",
                "broute",
                "-A",
                "BROUTING",
                "-p",
                "IPv4",
                "--ip-proto",
                "6",
                "--ip-dport",
                "!",
                "22",
                "--ip-sport",
                "!",
                "22",
                "-j",
                "redirect",
                "--redirect-target",
                "DROP",
            ],
        ),
    ]
}

pub fn set_iptables_safe<'a>(netns: &'a str, ip: &'a str) -> Vec<Vec<&'a str>> {
    vec![
        ip_netns(
            netns,
            vec![
                "iptables",
                "-t",
                "mangle",
                "-I",
                "PREROUTING",
                "-p",
                "tcp",
                "--dport",
                "81:1025",
                "-s",
                ip,
                "-j",
                "ACCEPT",
            ],
        ),
        ip_netns(
            netns,
            vec![
                "iptables",
                "-t",
                "mangle",
                "-I",
                "PREROUTING",
                "-p",
                "tcp",
                "--sport",
                "81:1025",
                "-d",
                ip,
                "-j",
                "ACCEPT",
            ],
        ),
        ip_netns(
            netns,
            vec![
                "iptables",
                "-t",
                "mangle",
                "-I",
                "PREROUTING",
                "-p",
                "tcp",
                "--dport",
                "1:81",
                "-s",
                ip,
                "-j",
                "ACCEPT",
            ],
        ),
        ip_netns(
            netns,
            vec![
                "iptables",
                "-t",
                "mangle",
                "-I",
                "PREROUTING",
                "-p",
                "tcp",
                "--sport",
                "1:81",
                "-d",
                ip,
                "-j",
                "ACCEPT",
            ],
        ),
    ]
}

#[derive(Debug)]
pub(super) struct OuterNatChain {
    pub chain: String,
    pub device: String,
    pub linked: bool,
}

impl OuterNatChain {
    pub fn create_command(chain: &str) -> Vec<&str> {
        vec!["ebtables", "-t", "nat", "-N", chain]
    }

    pub fn append_dnat<'a>(&'a self, destination_mac: &'a str) -> Vec<&'a str> {
        vec![
            "ebtables",
            "-t",
            "nat",
            "-A",
            &self.chain,
            "-j",
            "dnat",
            "--to-dst",
            destination_mac,
            "--dnat-target",
            "ACCEPT",
        ]
    }

    pub fn append_jump(&self) -> Vec<&str> {
        self.jump_command("-A")
    }

    pub fn delete_jump(&self) -> Vec<&str> {
        self.jump_command("-D")
    }

    pub fn flush(&self) -> Vec<&str> {
        vec!["ebtables", "-t", "nat", "-F", &self.chain]
    }

    pub fn delete_chain(&self) -> Vec<&str> {
        vec!["ebtables", "-t", "nat", "-X", &self.chain]
    }

    fn jump_command<'a>(&'a self, operation: &'a str) -> Vec<&'a str> {
        vec![
            "ebtables",
            "-t",
            "nat",
            operation,
            "PREROUTING",
            "-i",
            &self.device,
            "-j",
            &self.chain,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_and_safe_modes_install_one_jump_to_the_owned_chain() {
        let chain = OuterNatChain {
            chain: "tproxy-owned-ns".into(),
            device: "eth0".into(),
            linked: false,
        };
        for safe in [false, true] {
            let mut commands = set_iptables("owned-ns", Some("8080"), "12345");
            if safe {
                commands.extend(set_iptables_safe("owned-ns", "192.0.2.4/24"));
            }
            commands.extend([
                OuterNatChain::create_command(&chain.chain),
                chain.append_dnat("02:00:00:00:00:04"),
                chain.append_jump(),
            ]);
            let outer: Vec<_> = commands
                .iter()
                .filter(|cmd| cmd[0] == "ebtables")
                .cloned()
                .collect();
            assert_eq!(
                outer,
                vec![
                    vec!["ebtables", "-t", "nat", "-N", "tproxy-owned-ns"],
                    vec![
                        "ebtables",
                        "-t",
                        "nat",
                        "-A",
                        "tproxy-owned-ns",
                        "-j",
                        "dnat",
                        "--to-dst",
                        "02:00:00:00:00:04",
                        "--dnat-target",
                        "ACCEPT"
                    ],
                    vec![
                        "ebtables",
                        "-t",
                        "nat",
                        "-A",
                        "PREROUTING",
                        "-i",
                        "eth0",
                        "-j",
                        "tproxy-owned-ns"
                    ]
                ]
            );
        }
    }

    #[test]
    fn cleanup_commands_target_only_the_owned_chain_and_jump() {
        let chain = OuterNatChain {
            chain: "tproxy-owned-ns".into(),
            device: "eth0".into(),
            linked: true,
        };
        assert_eq!(
            chain.delete_jump(),
            vec![
                "ebtables",
                "-t",
                "nat",
                "-D",
                "PREROUTING",
                "-i",
                "eth0",
                "-j",
                "tproxy-owned-ns"
            ]
        );
        assert_eq!(
            chain.flush(),
            vec!["ebtables", "-t", "nat", "-F", "tproxy-owned-ns"]
        );
        assert_eq!(
            chain.delete_chain(),
            vec!["ebtables", "-t", "nat", "-X", "tproxy-owned-ns"]
        );
    }
}
