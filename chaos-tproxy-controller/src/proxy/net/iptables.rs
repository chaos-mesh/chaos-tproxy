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
pub(super) struct OuterNatRule {
    pub device: String,
    pub destination_mac: String,
}

impl OuterNatRule {
    pub fn append(&self) -> Vec<&str> {
        self.command("-A")
    }

    pub fn delete(&self) -> Vec<&str> {
        self.command("-D")
    }

    fn command<'a>(&'a self, operation: &'a str) -> Vec<&'a str> {
        vec![
            "ebtables",
            "-t",
            "nat",
            operation,
            "PREROUTING",
            "-i",
            &self.device,
            "-j",
            "dnat",
            "--to-dst",
            &self.destination_mac,
            "--dnat-target",
            "ACCEPT",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_and_safe_modes_install_only_one_owned_outer_rule() {
        let rule = OuterNatRule {
            device: "eth0".into(),
            destination_mac: "02:00:00:00:00:04".into(),
        };
        for safe in [false, true] {
            let mut commands = set_iptables("owned-ns", Some("8080"), "12345");
            if safe {
                commands.extend(set_iptables_safe("owned-ns", "192.0.2.4/24"));
            }
            commands.push(rule.append());
            let outer: Vec<_> = commands
                .iter()
                .filter(|cmd| cmd[0] == "ebtables")
                .cloned()
                .collect();
            assert_eq!(
                outer,
                vec![vec![
                    "ebtables",
                    "-t",
                    "nat",
                    "-A",
                    "PREROUTING",
                    "-i",
                    "eth0",
                    "-j",
                    "dnat",
                    "--to-dst",
                    "02:00:00:00:00:04",
                    "--dnat-target",
                    "ACCEPT"
                ]]
            );
            assert_eq!(commands.len(), if safe { 11 } else { 7 });
        }
        assert_eq!(
            rule.delete(),
            vec![
                "ebtables",
                "-t",
                "nat",
                "-D",
                "PREROUTING",
                "-i",
                "eth0",
                "-j",
                "dnat",
                "--to-dst",
                "02:00:00:00:00:04",
                "--dnat-target",
                "ACCEPT"
            ]
        );
    }
}
