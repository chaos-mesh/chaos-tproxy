package ptp

import (
	"context"
	"net"
	"net/netip"

	"github.com/chaos-mesh/chaos-tproxy/pkg/net/netutil"
	"github.com/pkg/errors"
	"github.com/vishvananda/netlink"
	"github.com/vishvananda/netns"
)

type addPlan struct {
	containerNetNS string
	sandboxNetNS   string

	containerLinkName string
	containerIfName   string
	sandboxIfName     string
	sandboxIP         netip.Prefix
	force             bool

	mtu                   int
	gateway               netip.Addr
	containerHardwareAddr net.HardwareAddr
}

func (c *linkPeerToPeer) Add(ctx context.Context, containerNetNS, sandboxNetNS, sandboxIfName string, opts ...Option) error {
	if err := validateAddArgs(containerNetNS, sandboxNetNS, sandboxIfName); err != nil {
		return err
	}

	plan, err := buildAddPlan(containerNetNS, sandboxNetNS, sandboxIfName, opts...)
	if err != nil {
		return err
	}

	if plan.force {
		if err := c.Del(ctx, containerNetNS, sandboxNetNS, sandboxIfName); err != nil {
			return err
		}
	} else {
		if err := ensureSandboxPeerAbsent(sandboxNetNS, sandboxIfName); err != nil {
			return err
		}
	}

	if err := createContainerPeer(plan); err != nil {
		return err
	}
	if err := configureSandboxPeer(plan); err != nil {
		_ = c.Del(context.Background(), containerNetNS, sandboxNetNS, sandboxIfName)
		return err
	}
	return nil
}

func buildAddPlan(containerNetNS, sandboxNetNS, sandboxIfName string, opts ...Option) (*addPlan, error) {
	o := defaultOptions()
	for _, opt := range opts {
		if opt != nil {
			opt(o)
		}
	}

	containerIfName := o.containerIfName
	if containerIfName == "" {
		containerIfName = defaultContainerIfName
	}
	if err := validateIfName(containerIfName, "container interface name"); err != nil {
		return nil, err
	}
	if o.sandboxIPSet {
		if err := validateSandboxIP(o.sandboxIP); err != nil {
			return nil, err
		}
	}

	plan := &addPlan{
		containerNetNS:    containerNetNS,
		sandboxNetNS:      sandboxNetNS,
		containerLinkName: containerIfName + "-p",
		containerIfName:   containerIfName,
		sandboxIfName:     sandboxIfName,
		sandboxIP:         o.sandboxIP,
		force:             o.force,
	}

	if err := netutil.WithNetNS(context.Background(), containerNetNS, func() error {
		link, err := netlink.LinkByName(containerIfName)
		if err != nil {
			return errors.Wrapf(err, "ptp.buildAddPlan look up container interface %s", containerIfName)
		}

		plan.mtu = link.Attrs().MTU
		plan.gateway, err = linkGateway(link)
		if err != nil {
			return err
		}
		return nil
	}); err != nil {
		return nil, err
	}
	return plan, nil
}

func ensureSandboxPeerAbsent(sandboxNetNS, sandboxIfName string) error {
	return netutil.WithNetNS(context.Background(), sandboxNetNS, func() error {
		if _, err := netlink.LinkByName(sandboxIfName); err != nil {
			if netutil.IsLinkNotFound(err) {
				return nil
			}
			return errors.Wrapf(err, "ptp.ensureSandboxPeerAbsent look up sandbox link %s", sandboxIfName)
		}
		return errors.Wrapf(ErrAlreadyExists, "ptp.ensureSandboxPeerAbsent sandbox link %s", sandboxIfName)
	})
}

func createContainerPeer(plan *addPlan) error {
	sandboxNS, err := netns.GetFromPath(plan.sandboxNetNS)
	if err != nil {
		return errors.Wrapf(err, "ptp.createContainerPeer open sandbox netns %s", plan.sandboxNetNS)
	}
	defer func() { _ = sandboxNS.Close() }()

	created := false
	if err := netutil.WithNetNS(context.Background(), plan.containerNetNS, func() error {
		veth := &netlink.Veth{
			LinkAttrs: netlink.LinkAttrs{
				Name:   plan.containerLinkName,
				MTU:    plan.mtu,
				TxQLen: 1000,
			},
			PeerName:      plan.sandboxIfName,
			PeerNamespace: netlink.NsFd(int(sandboxNS)),
			PeerMTU:       uint32(plan.mtu),
			PeerTxQLen:    1000,
		}
		if err := netlink.LinkAdd(veth); err != nil {
			return errors.Wrapf(err, "ptp.createContainerPeer create veth %s/%s", plan.containerLinkName, plan.sandboxIfName)
		}
		created = true

		link, err := netlink.LinkByName(plan.containerLinkName)
		if err != nil {
			return errors.Wrapf(err, "ptp.createContainerPeer look up created link %s", plan.containerLinkName)
		}
		plan.containerHardwareAddr = append(net.HardwareAddr(nil), link.Attrs().HardwareAddr...)
		if err := netlink.LinkSetUp(link); err != nil {
			return errors.Wrapf(err, "ptp.createContainerPeer set %s up", plan.containerLinkName)
		}
		return nil
	}); err != nil {
		if created {
			_ = (&linkPeerToPeer{}).Del(context.Background(), plan.containerNetNS, plan.sandboxNetNS, plan.sandboxIfName)
		}
		return err
	}
	return nil
}

func configureSandboxPeer(plan *addPlan) error {
	return netutil.WithNetNS(context.Background(), plan.sandboxNetNS, func() error {
		link, err := netlink.LinkByName(plan.sandboxIfName)
		if err != nil {
			return errors.Wrapf(err, "ptp.configureSandboxPeer look up sandbox link %s", plan.sandboxIfName)
		}
		if err := netlink.LinkSetMTU(link, plan.mtu); err != nil {
			return errors.Wrapf(err, "ptp.configureSandboxPeer set mtu on %s", plan.sandboxIfName)
		}
		if plan.sandboxIP.IsValid() {
			if err := replaceLinkAddr(link, plan.sandboxIP); err != nil {
				return err
			}
		}
		if err := netlink.LinkSetUp(link); err != nil {
			return errors.Wrapf(err, "ptp.configureSandboxPeer set %s up", plan.sandboxIfName)
		}
		if err := replaceSandboxDefaultRoute(plan.gateway, link.Attrs().Index, plan.sandboxIP.IsValid()); err != nil {
			return err
		}
		if plan.sandboxIP.IsValid() && len(plan.containerHardwareAddr) > 0 {
			if err := setSandboxGatewayNeighbor(plan.gateway, plan.containerHardwareAddr, link.Attrs().Index); err != nil {
				return err
			}
		}
		return nil
	})
}
