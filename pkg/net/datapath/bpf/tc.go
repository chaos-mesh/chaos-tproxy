package bpf

import (
	"context"
	"errors"
	"fmt"

	"github.com/chaos-mesh/chaos-tproxy/pkg/net/netutil"
	"github.com/cilium/ebpf"
	"github.com/vishvananda/netlink"
)

type tcDirection int

const (
	tcIngress tcDirection = iota
	tcEgress
)

type tcAttachment struct {
	nsPath   string
	ifName   string
	dir      tcDirection
	handle   uint32
	priority uint16
}

func attachTC(ctx context.Context, nsPath, ifName string, dir tcDirection, prog *ebpf.Program, handle uint32, priority uint16, name string) (tcAttachment, error) {
	attachment := tcAttachment{
		nsPath:   nsPath,
		ifName:   ifName,
		dir:      dir,
		handle:   handle,
		priority: priority,
	}
	err := netutil.WithNetNS(ctx, nsPath, func() error {
		link, err := netlink.LinkByName(ifName)
		if err != nil {
			return fmt.Errorf("look up %s: %w", ifName, err)
		}
		if err := ensureClsact(link); err != nil {
			return err
		}
		if err := detachTCFilter(link, dir, handle, priority); err != nil {
			return err
		}

		filter := &netlink.BpfFilter{
			FilterAttrs: netlink.FilterAttrs{
				LinkIndex: link.Attrs().Index,
				Parent:    tcParent(dir),
				Handle:    handle,
				Protocol:  0x0003, // ETH_P_ALL
				Priority:  priority,
			},
			Fd:           prog.FD(),
			Name:         name,
			DirectAction: true,
		}
		if err := netlink.FilterAdd(filter); err != nil {
			return fmt.Errorf("add %s filter on %s: %w", tcDirectionName(dir), ifName, err)
		}
		return nil
	})
	return attachment, err
}

func unbindAttachments(ctx context.Context, attachments []tcAttachment) error {
	var errs []error
	for _, attachment := range attachments {
		if err := detachTC(ctx, attachment); err != nil {
			errs = append(errs, err)
		}
	}
	return errors.Join(errs...)
}

func detachTC(ctx context.Context, attachment tcAttachment) error {
	return netutil.WithNetNS(ctx, attachment.nsPath, func() error {
		link, err := netlink.LinkByName(attachment.ifName)
		if err != nil {
			if netutil.IsLinkNotFound(err) {
				return nil
			}
			return fmt.Errorf("look up %s: %w", attachment.ifName, err)
		}
		return detachTCFilter(link, attachment.dir, attachment.handle, attachment.priority)
	})
}

func ensureClsact(link netlink.Link) error {
	qdisc := &netlink.GenericQdisc{
		QdiscAttrs: netlink.QdiscAttrs{
			LinkIndex: link.Attrs().Index,
			Handle:    netlink.MakeHandle(0xffff, 0),
			Parent:    netlink.HANDLE_CLSACT,
		},
		QdiscType: "clsact",
	}
	if err := netlink.QdiscAdd(qdisc); err != nil && !netutil.IsFileExists(err) {
		return fmt.Errorf("add clsact on %s: %w", link.Attrs().Name, err)
	}
	return nil
}

func detachTCFilter(link netlink.Link, dir tcDirection, handle uint32, priority uint16) error {
	filters, err := netlink.FilterList(link, tcParent(dir))
	if err != nil {
		return fmt.Errorf("list %s filters on %s: %w", tcDirectionName(dir), link.Attrs().Name, err)
	}

	var errs []error
	for _, filter := range filters {
		attrs := filter.Attrs()
		if attrs == nil {
			continue
		}
		if attrs.Handle != handle || attrs.Priority != priority {
			continue
		}
		if err := netlink.FilterDel(filter); err != nil {
			errs = append(errs, fmt.Errorf("delete %s filter on %s: %w", tcDirectionName(dir), link.Attrs().Name, err))
		}
	}
	return errors.Join(errs...)
}

func tcParent(dir tcDirection) uint32 {
	if dir == tcEgress {
		return uint32(netlink.HANDLE_MIN_EGRESS)
	}
	return uint32(netlink.HANDLE_MIN_INGRESS)
}

func tcDirectionName(dir tcDirection) string {
	if dir == tcEgress {
		return "egress"
	}
	return "ingress"
}
