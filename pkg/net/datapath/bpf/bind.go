package bpf

import (
	"context"
	"encoding/binary"
	"fmt"
	"net/netip"

	"github.com/cilium/ebpf"
	"github.com/cilium/ebpf/rlimit"

	"github.com/chaos-mesh/chaos-tproxy/pkg/net/datapath/bpf/bpfload"
)

// Bind loads the datapath BPF objects, configures maps, and attaches TC
// filters. The caller may Close the returned handle after Bind; attached TC
// filters keep kernel references until Unbind is called.
func Bind(ctx context.Context, opts Options) (*Handle, error) {
	opts = withDefaults(opts)
	if err := validateBindOptions(opts); err != nil {
		return nil, err
	}

	container, err := inspectLink(ctx, opts.Container)
	if err != nil {
		return nil, fmt.Errorf("inspect container interface: %w", err)
	}
	containerPeer, err := inspectLink(ctx, opts.ContainerPeer)
	if err != nil {
		return nil, fmt.Errorf("inspect container peer interface: %w", err)
	}
	sandboxPeer, err := inspectLink(ctx, opts.SandboxPeer)
	if err != nil {
		return nil, fmt.Errorf("inspect sandbox peer interface: %w", err)
	}
	sandboxLoopback, err := inspectLink(ctx, Interface{NetNS: opts.SandboxPeer.NetNS, Name: "lo"})
	if err != nil {
		return nil, fmt.Errorf("inspect sandbox loopback: %w", err)
	}

	targetIP := opts.TargetIP
	if !targetIP.IsValid() {
		targetIP = container.ip
	}
	if !targetIP.IsValid() || !targetIP.Is4() {
		return nil, fmt.Errorf("bpf bind: target IPv4 is required")
	}

	if err := rlimit.RemoveMemlock(); err != nil {
		return nil, fmt.Errorf("remove memlock limit: %w", err)
	}

	objects := &bpfload.DatapathObjects{}
	if err := bpfload.LoadDatapathObjects(objects, nil); err != nil {
		return nil, fmt.Errorf("load datapath BPF objects: %w", err)
	}

	handle := &Handle{opts: opts, objects: objects}
	success := false
	defer func() {
		if !success {
			_ = handle.Unbind(context.Background())
		}
	}()

	if err := configureMaps(objects, opts, targetIP, container, containerPeer, sandboxPeer, sandboxLoopback); err != nil {
		return nil, err
	}

	attachments, err := attachPrograms(ctx, opts, objects)
	if err != nil {
		return nil, err
	}
	handle.attachments = attachments
	success = true
	return handle, nil
}

// Unbind detaches the datapath TC filters from the interfaces described by
// opts. It does not require the original Bind process to still be running.
func Unbind(ctx context.Context, opts Options) error {
	opts = withDefaults(opts)
	if err := validateUnbindOptions(opts); err != nil {
		return err
	}
	return unbindAttachments(ctx, plannedAttachments(opts))
}

func configureMaps(objects *bpfload.DatapathObjects, opts Options, targetIP netip.Addr, container, containerPeer, sandboxPeer, sandboxLoopback linkInfo) error {
	rawTargetIP := targetIP.As4()
	value := bpfload.DatapathDatapathParams{
		ProxyMark:            opts.ProxyMark,
		TargetIp:             binary.LittleEndian.Uint32(rawTargetIP[:]),
		ContainerPeerIfindex: uint32(containerPeer.index),
		ContainerIfindex:     uint32(container.index),
		SandboxLoIfindex:     uint32(sandboxLoopback.index),
	}
	copy(value.SandboxPeerMac[:], sandboxPeer.mac)
	copy(value.ContainerMac[:], container.mac)

	if err := objects.ParamsMap.Update(uint32(0), value, ebpf.UpdateAny); err != nil {
		return fmt.Errorf("update params_map: %w", err)
	}

	enabled := uint8(1)
	for _, port := range opts.ProxyPorts {
		key := (port << 8) | (port >> 8)
		if err := objects.ProxyPorts.Update(key, enabled, ebpf.UpdateAny); err != nil {
			return fmt.Errorf("update proxy_ports[%d]: %w", port, err)
		}
	}
	return nil
}

func attachPrograms(ctx context.Context, opts Options, objects *bpfload.DatapathObjects) ([]tcAttachment, error) {
	specs := []struct {
		iface Interface
		dir   tcDirection
		prog  *ebpf.Program
		name  string
	}{
		{iface: opts.Container, dir: tcIngress, prog: objects.TcContainerIngress, name: "chaos_container_ingress"},
		{iface: opts.Container, dir: tcEgress, prog: objects.TcContainerEgress, name: "chaos_container_egress"},
		{iface: opts.ContainerPeer, dir: tcIngress, prog: objects.TcContainerPeerIngress, name: "chaos_container_peer_ingress"},
		{iface: opts.SandboxPeer, dir: tcIngress, prog: objects.TcSubSandboxIngress, name: "chaos_sub_sandbox_ingress"},
	}

	attachments := make([]tcAttachment, 0, len(specs))
	for _, spec := range specs {
		attachment, err := attachTC(ctx, spec.iface.NetNS, spec.iface.Name, spec.dir, spec.prog, opts.FilterHandle, opts.FilterPriority, spec.name)
		if err != nil {
			_ = unbindAttachments(context.Background(), attachments)
			return nil, err
		}
		attachments = append(attachments, attachment)
	}
	return attachments, nil
}

func plannedAttachments(opts Options) []tcAttachment {
	return []tcAttachment{
		{nsPath: opts.Container.NetNS, ifName: opts.Container.Name, dir: tcIngress, handle: opts.FilterHandle, priority: opts.FilterPriority},
		{nsPath: opts.Container.NetNS, ifName: opts.Container.Name, dir: tcEgress, handle: opts.FilterHandle, priority: opts.FilterPriority},
		{nsPath: opts.ContainerPeer.NetNS, ifName: opts.ContainerPeer.Name, dir: tcIngress, handle: opts.FilterHandle, priority: opts.FilterPriority},
		{nsPath: opts.SandboxPeer.NetNS, ifName: opts.SandboxPeer.Name, dir: tcIngress, handle: opts.FilterHandle, priority: opts.FilterPriority},
	}
}

func withDefaults(opts Options) Options {
	if opts.ContainerPeer.NetNS == "" {
		opts.ContainerPeer.NetNS = opts.Container.NetNS
	}
	if opts.ProxyMark == 0 {
		opts.ProxyMark = DefaultProxyMark
	}
	if opts.FilterHandle == 0 {
		opts.FilterHandle = DefaultFilterHandle
	}
	if opts.FilterPriority == 0 {
		opts.FilterPriority = DefaultFilterPriority
	}
	return opts
}

func validateBindOptions(opts Options) error {
	if err := validateUnbindOptions(opts); err != nil {
		return err
	}
	if !opts.TargetIP.IsValid() {
		return nil
	}
	if !opts.TargetIP.Is4() {
		return fmt.Errorf("bpf bind: target IP must be IPv4")
	}
	return nil
}

func validateUnbindOptions(opts Options) error {
	for name, iface := range map[string]Interface{
		"container":      opts.Container,
		"container peer": opts.ContainerPeer,
		"sandbox peer":   opts.SandboxPeer,
	} {
		if iface.NetNS == "" {
			return fmt.Errorf("bpf bind: %s netns is required", name)
		}
		if iface.Name == "" {
			return fmt.Errorf("bpf bind: %s interface name is required", name)
		}
	}
	return nil
}
