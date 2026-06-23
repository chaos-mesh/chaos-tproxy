// SPDX-License-Identifier: GPL-2.0
//
// PTP-backed chaos-tproxy datapath prototype.
//
// Section names describe roles instead of concrete device names:
//
//   tc/container_ingress       target container service interface ingress
//   tc/container_egress        target container service interface egress
//   tc/container_peer_ingress  container-side PTP veth ingress
//   tc/sub_sandbox_ingress     sandbox-side PTP veth ingress

#include <linux/bpf.h>
#include <linux/if_ether.h>
#include <linux/in.h>
#include <linux/ip.h>
#include <linux/pkt_cls.h>
#include <linux/tcp.h>
#include <stddef.h>

#include <bpf/bpf_endian.h>
#include <bpf/bpf_helpers.h>

#ifndef ETH_HLEN
#define ETH_HLEN 14
#endif

#ifndef BPF_F_INGRESS
#define BPF_F_INGRESS 1ULL
#endif

#define TPROXY_MARK 0x8000000u

struct datapath_params {
	__u32 proxy_mark;
	__u32 target_ip;
	__u32 container_peer_ifindex;
	__u32 container_ifindex;
	__u32 sandbox_lo_ifindex;
	__u8  sandbox_peer_mac[6];
	__u8  _pad0[2];
	__u8  container_mac[6];
	__u8  _pad1[2];
};

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(max_entries, 1);
	__type(key, __u32);
	__type(value, struct datapath_params);
} params_map SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, 64);
	__type(key, __u16);
	__type(value, __u8);
} proxy_ports SEC(".maps");

static __always_inline struct datapath_params *get_params(void)
{
	__u32 k = 0;
	return bpf_map_lookup_elem(&params_map, &k);
}

static __always_inline int port_is_proxied(__u16 port_be)
{
	return bpf_map_lookup_elem(&proxy_ports, &port_be) ? 1 : 0;
}

static __always_inline int parse_v4(struct __sk_buff *skb,
				    __u32 *saddr, __u32 *daddr,
				    __u8 *proto, __u16 *sport,
				    __u16 *dport, __u8 *tcp_flags)
{
	void *data = (void *)(long)skb->data;
	void *data_end = (void *)(long)skb->data_end;

	if (data + ETH_HLEN > data_end)
		return 0;

	struct ethhdr *eth = data;
	if (eth->h_proto != bpf_htons(ETH_P_IP))
		return 0;

	struct iphdr *iph = data + ETH_HLEN;
	if ((void *)(iph + 1) > data_end)
		return 0;

	*saddr = iph->saddr;
	*daddr = iph->daddr;
	*proto = iph->protocol;
	*sport = 0;
	*dport = 0;
	*tcp_flags = 0;

	if (iph->protocol != IPPROTO_TCP)
		return 1;

	__u32 ihl = iph->ihl * 4;
	if (ihl < sizeof(*iph))
		return 0;

	struct tcphdr *tcph = (void *)iph + ihl;
	if ((void *)(tcph + 1) > data_end)
		return 0;

	*sport = tcph->source;
	*dport = tcph->dest;

	__u8 *flags_byte = (void *)tcph + 13;
	if ((void *)(flags_byte + 1) > data_end)
		return 0;
	*tcp_flags = *flags_byte;
	return 1;
}

static __always_inline int should_capture(struct datapath_params *p,
					  __u32 saddr, __u32 daddr,
					  __u8 proto, __u16 sport,
					  __u16 dport)
{
	if (proto != IPPROTO_TCP)
		return 0;

	return ((daddr == p->target_ip) && port_is_proxied(dport)) ||
	       ((saddr == p->target_ip) && port_is_proxied(sport));
}

static __always_inline int from_proxy_magic(struct ethhdr *eth)
{
	return eth->h_source[0] == 0x02 &&
	       eth->h_source[1] == 0xce &&
	       eth->h_source[2] == 0x05 &&
	       eth->h_source[3] == 0xc1 &&
	       eth->h_source[4] == 0xc1 &&
	       eth->h_source[5] == 0xc1;
}

static __always_inline int redirect_to_sandbox(struct __sk_buff *skb,
					       struct datapath_params *p)
{
	if (bpf_skb_store_bytes(skb, offsetof(struct ethhdr, h_dest),
				p->sandbox_peer_mac, 6, 0) < 0)
		return TC_ACT_OK;
	return bpf_redirect(p->container_peer_ifindex, 0);
}

SEC("tc/container_ingress")
int tc_container_ingress(struct __sk_buff *skb)
{
	struct datapath_params *p = get_params();
	if (!p)
		return TC_ACT_OK;

	void *data = (void *)(long)skb->data;
	void *data_end = (void *)(long)skb->data_end;
	if (data + ETH_HLEN > data_end)
		return TC_ACT_OK;

	struct ethhdr *eth = data;
	if (from_proxy_magic(eth))
		return TC_ACT_OK;

	__u32 saddr, daddr;
	__u16 sport, dport;
	__u8 proto, flags;
	if (!parse_v4(skb, &saddr, &daddr, &proto, &sport, &dport, &flags))
		return TC_ACT_OK;

	if (!should_capture(p, saddr, daddr, proto, sport, dport))
		return TC_ACT_OK;

	return redirect_to_sandbox(skb, p);
}

SEC("tc/container_egress")
int tc_container_egress(struct __sk_buff *skb)
{
	struct datapath_params *p = get_params();
	if (!p)
		return TC_ACT_OK;

	__u32 saddr, daddr;
	__u16 sport, dport;
	__u8 proto, flags;
	if (!parse_v4(skb, &saddr, &daddr, &proto, &sport, &dport, &flags))
		return TC_ACT_OK;

	if (skb->mark == p->proxy_mark)
		return TC_ACT_OK;

	if (!should_capture(p, saddr, daddr, proto, sport, dport))
		return TC_ACT_OK;

	return redirect_to_sandbox(skb, p);
}

SEC("tc/container_peer_ingress")
int tc_container_peer_ingress(struct __sk_buff *skb)
{
	struct datapath_params *p = get_params();
	if (!p)
		return TC_ACT_OK;

	__u32 saddr, daddr;
	__u16 sport, dport;
	__u8 proto, flags;
	if (!parse_v4(skb, &saddr, &daddr, &proto, &sport, &dport, &flags))
		return bpf_redirect(p->container_ifindex, 0);

	if (daddr == p->target_ip) {
		__u8 magic_smac[6] = {0x02, 0xce, 0x05, 0xc1, 0xc1, 0xc1};

		(void)bpf_skb_store_bytes(skb, offsetof(struct ethhdr, h_source),
					  magic_smac, 6, 0);
		(void)bpf_skb_store_bytes(skb, offsetof(struct ethhdr, h_dest),
					  p->container_mac, 6, 0);
		return bpf_redirect(p->container_ifindex, BPF_F_INGRESS);
	}

	if (saddr == p->target_ip)
		skb->mark = p->proxy_mark;

	return bpf_redirect(p->container_ifindex, 0);
}

SEC("tc/sub_sandbox_ingress")
int tc_sub_sandbox_ingress(struct __sk_buff *skb)
{
	__u32 saddr, daddr;
	__u16 sport, dport;
	__u8 proto, flags;
	if (!parse_v4(skb, &saddr, &daddr, &proto, &sport, &dport, &flags))
		return TC_ACT_OK;

	skb->mark = TPROXY_MARK;
	return TC_ACT_OK;
}

char _license[] SEC("license") = "GPL";
