package ptp

import (
	"net"
	"net/netip"

	"github.com/chaos-mesh/chaos-tproxy/pkg/net/netutil"
	"github.com/pkg/errors"
	"github.com/vishvananda/netlink"
)

func linkGateway(link netlink.Link) (netip.Addr, error) {
	if addr, err := netutil.FirstGlobalUnicastIPv4(link); err != nil {
		return netip.Addr{}, err
	} else if addr.IsValid() {
		return addr, nil
	}
	return netip.Addr{}, errors.Wrapf(ErrInvalidConfig, "ptp.linkGateway no IPv4 address on %s", link.Attrs().Name)
}

func replaceLinkAddr(link netlink.Link, prefix netip.Prefix) error {
	addr := &netlink.Addr{IPNet: prefixToIPNet(prefix)}
	if err := netlink.AddrReplace(link, addr); err != nil {
		return errors.Wrapf(err, "ptp.replaceLinkAddr replace %s on %s", prefix.String(), link.Attrs().Name)
	}
	return nil
}

func replaceSandboxDefaultRoute(gateway netip.Addr, linkIndex int, viaGateway bool) error {
	route := &netlink.Route{
		LinkIndex: linkIndex,
		Dst:       defaultRouteDst(),
	}
	if viaGateway {
		route.Gw = ipFromAddr(gateway)
		route.Flags = int(netlink.FLAG_ONLINK)
	} else {
		route.Scope = netlink.SCOPE_LINK
	}
	if err := netlink.RouteReplace(route); err != nil {
		return errors.Wrapf(err, "ptp.replaceSandboxDefaultRoute replace sandbox default route")
	}
	return nil
}

func setSandboxGatewayNeighbor(gateway netip.Addr, hw net.HardwareAddr, linkIndex int) error {
	neigh := &netlink.Neigh{
		LinkIndex:    linkIndex,
		IP:           ipFromAddr(gateway),
		HardwareAddr: hw,
		State:        netlink.NUD_PERMANENT,
	}
	if err := netlink.NeighSet(neigh); err != nil {
		return errors.Wrapf(err, "ptp.setSandboxGatewayNeighbor set neighbor %s", gateway.String())
	}
	return nil
}

func defaultRouteDst() *net.IPNet {
	return &net.IPNet{IP: net.IPv4zero.To4(), Mask: net.CIDRMask(0, 32)}
}

func prefixToIPNet(prefix netip.Prefix) *net.IPNet {
	addr := prefix.Addr()
	return &net.IPNet{
		IP:   net.IP(addr.AsSlice()).To4(),
		Mask: net.CIDRMask(prefix.Bits(), 32),
	}
}

func ipFromAddr(addr netip.Addr) net.IP {
	return net.IP(addr.AsSlice()).To4()
}
