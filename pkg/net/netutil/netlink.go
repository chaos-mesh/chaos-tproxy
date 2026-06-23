package netutil

import (
	stderrors "errors"
	"fmt"
	"net/netip"
	"os"
	"strings"

	"github.com/vishvananda/netlink"
	"golang.org/x/sys/unix"
)

// IsLinkNotFound reports whether err is a netlink link-not-found error.
func IsLinkNotFound(err error) bool {
	if err == nil {
		return false
	}
	var notFound netlink.LinkNotFoundError
	return stderrors.As(err, &notFound) || strings.EqualFold(err.Error(), "link not found")
}

// IsFileExists reports whether err is an EEXIST-style netlink error.
func IsFileExists(err error) bool {
	if err == nil {
		return false
	}
	return stderrors.Is(err, os.ErrExist) ||
		stderrors.Is(err, unix.EEXIST) ||
		strings.EqualFold(err.Error(), "file exists")
}

// IsNetNSNotFound reports whether err indicates a missing network namespace path.
func IsNetNSNotFound(err error) bool {
	return stderrors.Is(err, os.ErrNotExist)
}

// FirstGlobalUnicastIPv4 returns the first global-unicast IPv4 address on link.
func FirstGlobalUnicastIPv4(link netlink.Link) (netip.Addr, error) {
	return firstLinkAddr(link, netlink.FAMILY_V4, func(addr netip.Addr) bool {
		return addr.Is4() && addr.IsGlobalUnicast()
	})
}

func firstLinkAddr(link netlink.Link, family int, accept func(netip.Addr) bool) (netip.Addr, error) {
	addrs, err := netlink.AddrList(link, family)
	if err != nil {
		return netip.Addr{}, fmt.Errorf("list addresses on %s: %w", link.Attrs().Name, err)
	}
	for _, addr := range addrs {
		parsed, ok := netip.AddrFromSlice(addr.IP)
		if !ok {
			continue
		}
		parsed = parsed.Unmap()
		if accept == nil || accept(parsed) {
			return parsed, nil
		}
	}
	return netip.Addr{}, nil
}
