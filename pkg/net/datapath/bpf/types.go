// Package bpf binds the chaos-tproxy eBPF datapath to an existing PTP
// topology. It owns only eBPF object loading, map configuration, and TC
// filter attachment; namespace and veth creation stay outside this package.
package bpf

import (
	"context"
	"net/netip"

	"github.com/chaos-mesh/chaos-tproxy/pkg/net/datapath/bpf/bpfload"
)

const (
	DefaultProxyMark      uint32 = 0xCFC1
	DefaultFilterHandle   uint32 = 0xCFC1
	DefaultFilterPriority uint16 = 2023
)

// Interface identifies an interface inside a Linux network namespace.
type Interface struct {
	NetNS string
	Name  string
}

// Options describes where the datapath should be attached.
type Options struct {
	Container     Interface
	ContainerPeer Interface
	SandboxPeer   Interface

	// TargetIP is the service/container IP. When unset, Bind detects the
	// first non-loopback IPv4 address on Container.Name.
	TargetIP netip.Addr

	// ProxyPorts contains TCP destination ports to redirect.
	ProxyPorts []uint16
	ProxyMark  uint32

	FilterHandle   uint32
	FilterPriority uint16
}

// Handle keeps userspace references to loaded BPF objects. Closing it releases
// process-owned fds but leaves TC filters bound; call Unbind to detach filters.
type Handle struct {
	opts        Options
	objects     *bpfload.DatapathObjects
	attachments []tcAttachment
}

// Close releases userspace BPF fds. TC filters stay attached until Unbind is
// called, which is useful for the CLI bind command.
func (h *Handle) Close() error {
	if h == nil || h.objects == nil {
		return nil
	}
	err := h.objects.Close()
	h.objects = nil
	return err
}

// Unbind detaches the TC filters for this handle and releases userspace fds.
func (h *Handle) Unbind(ctx context.Context) error {
	if h == nil {
		return nil
	}
	err := unbindAttachments(ctx, h.attachments)
	closeErr := h.Close()
	if err != nil {
		return err
	}
	return closeErr
}
