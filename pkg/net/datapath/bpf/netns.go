package bpf

import (
	"context"
	"fmt"
	"net"
	"net/netip"

	"github.com/chaos-mesh/chaos-tproxy/pkg/net/netutil"
	"github.com/vishvananda/netlink"
)

type linkInfo struct {
	index int
	mac   net.HardwareAddr
	ip    netip.Addr
}

func inspectLink(ctx context.Context, iface Interface) (linkInfo, error) {
	var info linkInfo
	err := netutil.WithNetNS(ctx, iface.NetNS, func() error {
		link, err := netlink.LinkByName(iface.Name)
		if err != nil {
			return fmt.Errorf("look up %s: %w", iface.Name, err)
		}
		info.index = link.Attrs().Index
		info.mac = append(net.HardwareAddr(nil), link.Attrs().HardwareAddr...)

		ip, err := netutil.FirstGlobalUnicastIPv4(link)
		if err != nil {
			return err
		}
		info.ip = ip
		return nil
	})
	return info, err
}
