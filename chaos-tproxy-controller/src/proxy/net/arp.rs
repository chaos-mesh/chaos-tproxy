use std::net::Ipv4Addr;

use libarp::arp::{ArpMessage, Operation};
use libarp::interfaces::{Interface, MacAddr};
use pnet::packet::ethernet::EtherTypes;

pub fn gratuitous_arp(interface: Interface, ip_addr: Ipv4Addr, mac_addr: MacAddr) {
    let arp_request = ArpMessage::new(
        EtherTypes::Arp,
        mac_addr,
        ip_addr,
        MacAddr(0xff, 0xff, 0xff, 0xff, 0xff, 0xff),
        ip_addr,
        Operation::ArpRequest,
    );
    arp_request
        .send(&interface)
        .unwrap_or_else(|e| tracing::error!("gratuitous arp send fail : {}", e));
}
