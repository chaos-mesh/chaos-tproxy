// Package net builds the sibling-netns + veth network plumbing used by
// chaos-tproxy. The builder runs from inside the target container's
// network namespace and produces:
//
//   - a chaos/chaospeer veth pair (chaos in target ns, chaospeer in chaosns)
//   - a sibling netns ("chaosns") with link-local routing toward target ns
//   - fwmark-based policy routing inside chaosns so tproxy traffic lands on lo
//   - permissive sysctls in both namespaces
//
// It does NOT load eBPF programs or spawn the proxy — those are the
// caller's responsibility. The returned *Network exposes the link/ns
// handles needed by those callers.
package net

import (
	"errors"
	"fmt"
	"net"
	"os"
	"runtime"

	"github.com/vishvananda/netlink"
	"github.com/vishvananda/netns"
	"golang.org/x/sys/unix"
)

const (
	defaultIfaceWAN     = "eth0"
	defaultHostVethName = "chaos"
	defaultPeerVethName = "chaospeer"
	// chaosNsDir is a chaos-tproxy-owned subdirectory under the system
	// netns dir. Each injection bind-mounts its netns to
	// <chaosNsDir>/<id>, isolating us from `ip netns` state and from
	// other concurrent injections. Concurrent injections must pass
	// distinct ids via WithChaosNsID (typically container-derived).
	chaosNsDir            = "/var/run/netns/chaosns"
	defaultPeerLinkLocal  = "169.254.0.11"
	defaultGwLinkLocal    = "169.254.0.1"
	defaultTproxyMark     = 0x8000000
	defaultTproxyRouteTbl = 2023
)

// NetworkBuilder assembles the chaos sibling-netns network. All WithX
// methods are optional; defaults match the layout described in the
// package doc. Call Build to materialize, then *Network.Teardown to undo.
type NetworkBuilder struct {
	ifaceWAN       string
	chaosNsID    string
	hostVethName   string
	peerVethName   string
	peerLinkLocal  string
	gwLinkLocal    string
	tproxyMark     uint32
	tproxyRouteTbl int
}

// NewNetworkBuilder returns a builder for the chaos sibling-netns
// network. id is the per-injection identifier used as the leaf of the
// bind-mount path (/var/run/netns/chaosns/<id>) and must be unique
// across concurrent injections on the same host (typically derived
// from container id).
func NewNetworkBuilder(id string) *NetworkBuilder {
	return &NetworkBuilder{
		chaosNsID:      id,
		ifaceWAN:       defaultIfaceWAN,
		hostVethName:   defaultHostVethName,
		peerVethName:   defaultPeerVethName,
		peerLinkLocal:  defaultPeerLinkLocal,
		gwLinkLocal:    defaultGwLinkLocal,
		tproxyMark:     defaultTproxyMark,
		tproxyRouteTbl: defaultTproxyRouteTbl,
	}
}

func (b *NetworkBuilder) WithIfaceWAN(name string) *NetworkBuilder {
	b.ifaceWAN = name
	return b
}

func (b *NetworkBuilder) WithVethNames(host, peer string) *NetworkBuilder {
	b.hostVethName = host
	b.peerVethName = peer
	return b
}

func (b *NetworkBuilder) WithLinkLocal(peerIP, gwIP string) *NetworkBuilder {
	b.peerLinkLocal = peerIP
	b.gwLinkLocal = gwIP
	return b
}

func (b *NetworkBuilder) WithTproxyMark(mark uint32) *NetworkBuilder {
	b.tproxyMark = mark
	return b
}

func (b *NetworkBuilder) WithRouteTable(table int) *NetworkBuilder {
	b.tproxyRouteTbl = table
	return b
}

// Network is the materialized result of a successful Build. The caller
// must invoke Teardown to release veth, chaosns, and the locked OS thread.
type Network struct {
	cfg NetworkBuilder

	HostNs    netns.NsHandle
	ChaosNs   netns.NsHandle
	Chaos     netlink.Link // host-side veth, lives in target ns
	ChaosPeer netlink.Link // peer-side veth, lives in chaosns
	Eth0      netlink.Link
	ChaosMac  net.HardwareAddr
	PeerMac   net.HardwareAddr
}

// Build runs the full setup sequence. The caller must already be inside
// the target container's network namespace; Build will LockOSThread and
// leave the thread locked until Teardown.
func (b *NetworkBuilder) Build() (n *Network, err error) {
	if b.chaosNsID == "" {
		return nil, fmt.Errorf("net: chaos netns id is required (pass non-empty id to NewNetworkBuilder)")
	}

	runtime.LockOSThread()

	hostNs, err := netns.Get()
	if err != nil {
		runtime.UnlockOSThread()
		return nil, fmt.Errorf("get current netns: %w", err)
	}

	n = &Network{cfg: *b, HostNs: hostNs}

	// Roll back partially-built state on any error.
	defer func() {
		if err != nil {
			n.Teardown()
			n = nil
		}
	}()

	eth0, err := netlink.LinkByName(b.ifaceWAN)
	if err != nil {
		return nil, fmt.Errorf("look up %s: %w", b.ifaceWAN, err)
	}
	n.Eth0 = eth0

	// Best-effort cleanup of stale leftovers from a previous run.
	_ = netlink.LinkDel(&netlink.Veth{LinkAttrs: netlink.LinkAttrs{Name: b.hostVethName}})
	_ = deleteChaosNetns(b.chaosNsID)

	if err = netlink.LinkAdd(&netlink.Veth{
		LinkAttrs: netlink.LinkAttrs{Name: b.hostVethName, TxQLen: 1000},
		PeerName:  b.peerVethName,
	}); err != nil {
		return nil, fmt.Errorf("create veth %s<->%s: %w", b.hostVethName, b.peerVethName, err)
	}

	chaos, err := netlink.LinkByName(b.hostVethName)
	if err != nil {
		return nil, fmt.Errorf("look up %s: %w", b.hostVethName, err)
	}
	peer, err := netlink.LinkByName(b.peerVethName)
	if err != nil {
		return nil, fmt.Errorf("look up %s: %w", b.peerVethName, err)
	}
	n.Chaos = chaos
	n.ChaosPeer = peer
	n.ChaosMac = chaos.Attrs().HardwareAddr
	n.PeerMac = peer.Attrs().HardwareAddr

	if err = netlink.LinkSetUp(chaos); err != nil {
		return nil, fmt.Errorf("set %s up: %w", b.hostVethName, err)
	}

	chaosNs, err := createChaosNetns(b.chaosNsID)
	if err != nil {
		return nil, fmt.Errorf("create %s: %w", b.chaosNsID, err)
	}
	n.ChaosNs = chaosNs

	if err = netlink.LinkSetNsFd(peer, int(chaosNs)); err != nil {
		return nil, fmt.Errorf("move %s to %s: %w", b.peerVethName, b.chaosNsID, err)
	}

	if err = n.configureChaosNs(); err != nil {
		return nil, err
	}

	if err = configureTargetSysctls(b.ifaceWAN); err != nil {
		return nil, err
	}

	return n, nil
}

// configureChaosNs runs inside chaosns: addresses, routes, rules, sysctls.
func (n *Network) configureChaosNs() error {
	return n.withChaosNs(func() error {
		peer, err := netlink.LinkByName(n.cfg.peerVethName)
		if err != nil {
			return fmt.Errorf("%s in chaosns: %w", n.cfg.peerVethName, err)
		}
		if err := netlink.LinkSetUp(peer); err != nil {
			return fmt.Errorf("set %s up: %w", n.cfg.peerVethName, err)
		}
		lo, err := netlink.LinkByName("lo")
		if err != nil {
			return fmt.Errorf("lo in chaosns: %w", err)
		}
		if err := netlink.LinkSetUp(lo); err != nil {
			return fmt.Errorf("set lo up: %w", err)
		}

		peerIP := net.ParseIP(n.cfg.peerLinkLocal)
		if err := netlink.AddrAdd(peer, &netlink.Addr{
			IPNet: &net.IPNet{IP: peerIP, Mask: net.CIDRMask(32, 32)},
		}); err != nil && !errors.Is(err, unix.EEXIST) {
			return fmt.Errorf("addr add %s: %w", n.cfg.peerLinkLocal, err)
		}

		gwIP := net.ParseIP(n.cfg.gwLinkLocal)
		if err := netlink.RouteAdd(&netlink.Route{
			LinkIndex: peer.Attrs().Index,
			Dst:       &net.IPNet{IP: gwIP, Mask: net.CIDRMask(32, 32)},
			Scope:     netlink.SCOPE_LINK,
		}); err != nil && !errors.Is(err, unix.EEXIST) {
			return fmt.Errorf("route gw %s: %w", n.cfg.gwLinkLocal, err)
		}
		if err := netlink.RouteAdd(&netlink.Route{
			LinkIndex: peer.Attrs().Index,
			Dst:       &net.IPNet{IP: net.IPv4zero, Mask: net.CIDRMask(0, 32)},
			Gw:        gwIP,
		}); err != nil && !errors.Is(err, unix.EEXIST) {
			return fmt.Errorf("route default: %w", err)
		}

		// Static neigh: gw -> chaos (host-side) MAC, so chaosns egress
		// can hop to the target ns without ARP.
		if err := netlink.NeighSet(&netlink.Neigh{
			IP:           gwIP,
			HardwareAddr: n.ChaosMac,
			LinkIndex:    peer.Attrs().Index,
			State:        netlink.NUD_PERMANENT,
		}); err != nil {
			return fmt.Errorf("neigh set: %w", err)
		}

		if err := netlink.RouteAdd(&netlink.Route{
			LinkIndex: lo.Attrs().Index,
			Dst:       &net.IPNet{IP: net.IPv4zero, Mask: net.CIDRMask(0, 32)},
			Scope:     unix.RT_SCOPE_HOST,
			Type:      unix.RTN_LOCAL,
			Table:     n.cfg.tproxyRouteTbl,
		}); err != nil && !errors.Is(err, unix.EEXIST) {
			return fmt.Errorf("route local default table %d: %w", n.cfg.tproxyRouteTbl, err)
		}

		mark := n.cfg.tproxyMark
		rule := netlink.NewRule()
		rule.Family = unix.AF_INET
		rule.Table = n.cfg.tproxyRouteTbl
		rule.Mark = mark
		rule.Mask = &mark
		rule.Priority = 100
		if err := netlink.RuleAdd(rule); err != nil && !errors.Is(err, unix.EEXIST) {
			return fmt.Errorf("rule add fwmark: %w", err)
		}

		// Best-effort sysctls in chaosns.
		_ = writeSysctl("/proc/sys/net/ipv4/conf/all/rp_filter", "0")
		_ = writeSysctl("/proc/sys/net/ipv4/conf/"+n.cfg.peerVethName+"/rp_filter", "0")
		_ = writeSysctl("/proc/sys/net/ipv4/conf/all/accept_local", "1")
		_ = writeSysctl("/proc/sys/net/ipv4/conf/"+n.cfg.peerVethName+"/accept_local", "1")
		_ = writeSysctl("/proc/sys/net/ipv4/ip_forward", "1")
		return nil
	})
}

// withChaosNs runs fn while temporarily switched into chaosns.
func (n *Network) withChaosNs(fn func() error) error {
	if err := netns.Set(n.ChaosNs); err != nil {
		return fmt.Errorf("enter chaosns: %w", err)
	}
	defer func() { _ = netns.Set(n.HostNs) }()
	return fn()
}

// AddTargetARP installs a static ARP entry in the target ns so reply
// packets to a forged client source IP can be addressed without ARP.
func (n *Network) AddTargetARP(clientIP net.IP, clientMAC net.HardwareAddr) error {
	return netlink.NeighSet(&netlink.Neigh{
		IP:           clientIP,
		HardwareAddr: clientMAC,
		LinkIndex:    n.Eth0.Attrs().Index,
		State:        netlink.NUD_PERMANENT,
	})
}

// Teardown reverses Build. Safe to call after a partial Build failure
// and idempotent across repeated calls.
func (n *Network) Teardown() {
	if n == nil {
		return
	}
	if l, err := netlink.LinkByName(n.cfg.hostVethName); err == nil {
		_ = netlink.LinkDel(l)
	}
	_ = deleteChaosNetns(n.cfg.chaosNsID)
	if n.ChaosNs != 0 {
		_ = n.ChaosNs.Close()
		n.ChaosNs = 0
	}
	if n.HostNs != 0 {
		_ = n.HostNs.Close()
		n.HostNs = 0
	}
	runtime.UnlockOSThread()
}

func configureTargetSysctls(ifaceWAN string) error {
	pairs := []struct{ path, val string }{
		{"/proc/sys/net/ipv4/conf/all/rp_filter", "0"},
		{"/proc/sys/net/ipv4/conf/lo/rp_filter", "0"},
		{"/proc/sys/net/ipv4/conf/" + ifaceWAN + "/rp_filter", "0"},
		{"/proc/sys/net/ipv4/conf/all/accept_local", "1"},
		{"/proc/sys/net/ipv4/conf/lo/accept_local", "1"},
		{"/proc/sys/net/ipv4/conf/" + ifaceWAN + "/accept_local", "1"},
		{"/proc/sys/net/ipv4/ip_forward", "1"},
	}
	for _, kv := range pairs {
		_ = writeSysctl(kv.path, kv.val)
	}
	return nil
}

func createChaosNetns(id string) (netns.NsHandle, error) {
	if err := os.MkdirAll(chaosNsDir, 0o755); err != nil {
		return 0, err
	}
	path := chaosNsDir + "/" + id
	f, err := os.Create(path)
	if err != nil {
		return 0, err
	}
	if err := f.Close(); err != nil {
		return 0, fmt.Errorf("close netns mountpoint %s: %w", path, err)
	}

	prev, err := netns.Get()
	if err != nil {
		return 0, err
	}
	defer func() { _ = prev.Close() }()

	newNs, err := netns.New()
	if err != nil {
		return 0, fmt.Errorf("create new netns: %w", err)
	}
	if err := unix.Mount("/proc/self/ns/net", path, "none", unix.MS_BIND, ""); err != nil {
		_ = newNs.Close()
		_ = os.Remove(path)
		return 0, fmt.Errorf("bind mount netns to %s: %w", path, err)
	}
	if err := netns.Set(prev); err != nil {
		_ = newNs.Close()
		return 0, fmt.Errorf("restore prev ns: %w", err)
	}
	return newNs, nil
}

func deleteChaosNetns(id string) error {
	path := chaosNsDir + "/" + id
	_ = unix.Unmount(path, unix.MNT_DETACH)
	return os.Remove(path)
}

func writeSysctl(path, value string) error {
	return os.WriteFile(path, []byte(value), 0o644)
}
