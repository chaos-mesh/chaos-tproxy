package ptp

import (
	"context"

	"github.com/pkg/errors"
	"github.com/vishvananda/netlink"
)

func (c *linkPeerToPeer) Del(ctx context.Context, containerNetNS, sandboxNetNS, sandboxIfName string) error {
	if err := validateNetNSPair(containerNetNS, sandboxNetNS); err != nil {
		return err
	}
	if err := validateIfName(sandboxIfName, "sandbox interface name"); err != nil {
		return err
	}

	err := withNetNS(sandboxNetNS, func() error {
		link, err := netlink.LinkByName(sandboxIfName)
		if err != nil {
			if isLinkNotFound(err) {
				return nil
			}
			return err
		}
		if err := netlink.LinkDel(link); err != nil {
			return errors.Wrapf(err, "ptp.connector.Del delete %s", link.Attrs().Name)
		}
		return nil
	})
	if isNetNSNotFound(err) {
		return nil
	}
	return err
}
