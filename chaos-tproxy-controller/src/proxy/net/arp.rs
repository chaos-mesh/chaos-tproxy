use std::{io, net::Ipv4Addr};

use libarp::interfaces::{Interface, MacAddr};
use pnet::datalink::{self, Channel, Config};
use pnet::packet::arp::{ArpHardwareTypes, ArpOperations, MutableArpPacket};
use pnet::packet::ethernet::{EtherTypes, MutableEthernetPacket};
use pnet::packet::{MutablePacket, Packet};

pub fn gratuitous_arp(
    interface: Interface,
    ip_addr: Ipv4Addr,
    mac_addr: MacAddr,
) -> io::Result<()> {
    let source: pnet::util::MacAddr = mac_addr.into();
    let broadcast = pnet::util::MacAddr::broadcast();
    let mut buffer = vec![
        0;
        MutableEthernetPacket::minimum_packet_size()
            + MutableArpPacket::minimum_packet_size()
    ];
    let mut ethernet = MutableEthernetPacket::new(&mut buffer).unwrap();
    ethernet.set_destination(broadcast);
    ethernet.set_source(source);
    ethernet.set_ethertype(EtherTypes::Arp);
    let mut arp = MutableArpPacket::new(ethernet.payload_mut()).unwrap();
    arp.set_hardware_type(ArpHardwareTypes::Ethernet);
    arp.set_protocol_type(EtherTypes::Ipv4);
    arp.set_hw_addr_len(6);
    arp.set_proto_addr_len(4);
    arp.set_operation(ArpOperations::Request);
    arp.set_sender_hw_addr(source);
    arp.set_sender_proto_addr(ip_addr);
    arp.set_target_hw_addr(broadcast);
    arp.set_target_proto_addr(ip_addr);

    let config = Config {
        promiscuous: false,
        ..Config::default()
    };
    match datalink::channel(interface.get_raw_interface(), config)? {
        Channel::Ethernet(mut tx, _) => tx
            .send_to(ethernet.packet(), None)
            .ok_or_else(|| io::Error::other("ARP send buffer is too small"))?,
        _ => Err(io::Error::other("unsupported datalink channel for ARP")),
    }
}
