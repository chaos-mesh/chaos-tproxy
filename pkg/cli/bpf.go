package cli

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/netip"
	"strconv"
	"strings"
	"time"

	"github.com/chaos-mesh/chaos-tproxy/pkg/net/datapath/bpf"
	containerruntime "github.com/chaos-mesh/chaos-tproxy/pkg/runtime"
	"github.com/pkg/errors"
	"github.com/spf13/cobra"
)

type bpfOptions struct {
	runtimeName     string
	output          string
	timeout         time.Duration
	containerIfName string
	containerPeer   string
	targetIP        string
	proxyPorts      string
	proxyMark       uint32
	filterHandle    uint32
	filterPriority  uint16
}

type bpfResult struct {
	Operation       string   `json:"operation"`
	Container       string   `json:"container"`
	Runtime         string   `json:"runtime"`
	ContainerNetNS  string   `json:"containerNetNS"`
	SandboxNetNS    string   `json:"sandboxNetNS"`
	ContainerIfName string   `json:"containerIfName"`
	ContainerPeer   string   `json:"containerPeer"`
	SandboxIfName   string   `json:"sandboxIfName"`
	TargetIP        string   `json:"targetIP,omitempty"`
	ProxyPorts      []uint16 `json:"proxyPorts,omitempty"`
	ProxyMark       uint32   `json:"proxyMark"`
}

func newBPFCommand(streams IOStreams) *cobra.Command {
	opts := &bpfOptions{
		runtimeName:     containerruntime.RuntimeDocker,
		output:          "text",
		timeout:         10 * time.Second,
		containerIfName: "eth0",
		proxyMark:       bpf.DefaultProxyMark,
		filterHandle:    bpf.DefaultFilterHandle,
		filterPriority:  bpf.DefaultFilterPriority,
	}

	cmd := &cobra.Command{
		Use:   "bpf",
		Short: "Manage eBPF datapath bindings",
	}
	cmd.PersistentFlags().StringVar(&opts.runtimeName, "runtime", opts.runtimeName, "container runtime")
	cmd.PersistentFlags().StringVarP(&opts.output, "output", "o", opts.output, "output format: text or json")
	cmd.PersistentFlags().DurationVar(&opts.timeout, "timeout", opts.timeout, "runtime inspect timeout")
	cmd.PersistentFlags().StringVar(&opts.containerIfName, "container-iface", opts.containerIfName, "container service interface name")
	cmd.PersistentFlags().StringVar(&opts.containerPeer, "container-peer", opts.containerPeer, "container-side PTP peer interface name (default <container-iface>-p)")
	cmd.PersistentFlags().Uint32Var(&opts.filterHandle, "filter-handle", opts.filterHandle, "TC filter handle")
	cmd.PersistentFlags().Uint16Var(&opts.filterPriority, "filter-priority", opts.filterPriority, "TC filter priority")

	cmd.AddCommand(newBPFBindCommand(streams, opts))
	cmd.AddCommand(newBPFUnbindCommand(streams, opts))
	return cmd
}

func newBPFBindCommand(streams IOStreams, opts *bpfOptions) *cobra.Command {
	cmd := &cobra.Command{
		Use:   "bind CONTAINER SANDBOX_NETNS SANDBOX_IFACE",
		Short: "Bind the eBPF datapath to an existing PTP link",
		Args:  cobra.ExactArgs(3),
		RunE: func(cmd *cobra.Command, args []string) error {
			return runBPFBind(cmd.Context(), streams.Out, args[0], args[1], args[2], opts)
		},
	}
	cmd.Flags().StringVar(&opts.targetIP, "target-ip", opts.targetIP, "target IPv4 address (default auto-detect from container interface)")
	cmd.Flags().StringVar(&opts.proxyPorts, "proxy-ports", opts.proxyPorts, "comma-separated TCP destination ports to redirect")
	cmd.Flags().Uint32Var(&opts.proxyMark, "proxy-mark", opts.proxyMark, "SO_MARK value used by proxy-originated sockets")
	return cmd
}

func newBPFUnbindCommand(streams IOStreams, opts *bpfOptions) *cobra.Command {
	cmd := &cobra.Command{
		Use:   "unbind CONTAINER SANDBOX_NETNS SANDBOX_IFACE",
		Short: "Remove eBPF datapath TC filters",
		Args:  cobra.ExactArgs(3),
		RunE: func(cmd *cobra.Command, args []string) error {
			return runBPFUnbind(cmd.Context(), streams.Out, args[0], args[1], args[2], opts)
		},
	}
	return cmd
}

func runBPFBind(ctx context.Context, stdout io.Writer, containerID, sandboxNetNS, sandboxIfName string, opts *bpfOptions) error {
	info, ports, targetIP, bindOpts, err := buildBPFOptions(ctx, containerID, sandboxNetNS, sandboxIfName, opts)
	if err != nil {
		return err
	}

	handle, err := bpf.Bind(ctx, bindOpts)
	if err != nil {
		return errors.Wrapf(err, "cli.runBPFBind bind container %s", containerID)
	}
	if err := handle.Close(); err != nil {
		return errors.Wrap(err, "cli.runBPFBind close userspace bpf handles")
	}

	return writeBPFResult(stdout, bpfResult{
		Operation:       "bind",
		Container:       containerID,
		Runtime:         opts.runtimeName,
		ContainerNetNS:  info.NetNS,
		SandboxNetNS:    sandboxNetNS,
		ContainerIfName: opts.containerIfName,
		ContainerPeer:   effectiveContainerPeer(opts),
		SandboxIfName:   sandboxIfName,
		TargetIP:        targetIP,
		ProxyPorts:      ports,
		ProxyMark:       bindOpts.ProxyMark,
	}, opts.output)
}

func runBPFUnbind(ctx context.Context, stdout io.Writer, containerID, sandboxNetNS, sandboxIfName string, opts *bpfOptions) error {
	info, ports, targetIP, bindOpts, err := buildBPFOptions(ctx, containerID, sandboxNetNS, sandboxIfName, opts)
	if err != nil {
		return err
	}

	if err := bpf.Unbind(ctx, bindOpts); err != nil {
		return errors.Wrapf(err, "cli.runBPFUnbind unbind container %s", containerID)
	}

	return writeBPFResult(stdout, bpfResult{
		Operation:       "unbind",
		Container:       containerID,
		Runtime:         opts.runtimeName,
		ContainerNetNS:  info.NetNS,
		SandboxNetNS:    sandboxNetNS,
		ContainerIfName: opts.containerIfName,
		ContainerPeer:   effectiveContainerPeer(opts),
		SandboxIfName:   sandboxIfName,
		TargetIP:        targetIP,
		ProxyPorts:      ports,
		ProxyMark:       bindOpts.ProxyMark,
	}, opts.output)
}

func buildBPFOptions(ctx context.Context, containerID, sandboxNetNS, sandboxIfName string, opts *bpfOptions) (*containerruntime.ContainerInfo, []uint16, string, bpf.Options, error) {
	info, err := resolveBPFContainerInfo(ctx, containerID, opts)
	if err != nil {
		return nil, nil, "", bpf.Options{}, err
	}

	ports, err := parsePortList(opts.proxyPorts)
	if err != nil {
		return nil, nil, "", bpf.Options{}, err
	}

	var targetIP netip.Addr
	if opts.targetIP != "" {
		targetIP, err = netip.ParseAddr(opts.targetIP)
		if err != nil {
			return nil, nil, "", bpf.Options{}, errors.Wrapf(err, "cli.buildBPFOptions parse target ip %s", opts.targetIP)
		}
	}

	bindOpts := bpf.Options{
		Container: bpf.Interface{
			NetNS: info.NetNS,
			Name:  opts.containerIfName,
		},
		ContainerPeer: bpf.Interface{
			NetNS: info.NetNS,
			Name:  effectiveContainerPeer(opts),
		},
		SandboxPeer: bpf.Interface{
			NetNS: sandboxNetNS,
			Name:  sandboxIfName,
		},
		TargetIP:       targetIP,
		ProxyPorts:     ports,
		ProxyMark:      opts.proxyMark,
		FilterHandle:   opts.filterHandle,
		FilterPriority: opts.filterPriority,
	}

	return info, ports, opts.targetIP, bindOpts, nil
}

func resolveBPFContainerInfo(ctx context.Context, containerID string, opts *bpfOptions) (*containerruntime.ContainerInfo, error) {
	client, err := containerruntime.NewClient(opts.runtimeName, containerruntime.WithTimeout(opts.timeout))
	if err != nil {
		return nil, errors.Wrap(err, "cli.resolveBPFContainerInfo create runtime client")
	}

	info, err := client.ContainerInfo(ctx, containerID)
	if err != nil {
		return nil, errors.Wrapf(err, "cli.resolveBPFContainerInfo inspect container %s", containerID)
	}
	return info, nil
}

func parsePortList(raw string) ([]uint16, error) {
	if strings.TrimSpace(raw) == "" {
		return nil, nil
	}
	parts := strings.Split(raw, ",")
	ports := make([]uint16, 0, len(parts))
	for _, part := range parts {
		part = strings.TrimSpace(part)
		if part == "" {
			continue
		}
		parsed, err := strconv.ParseUint(part, 10, 16)
		if err != nil {
			return nil, errors.Wrapf(err, "cli.parsePortList parse %q", part)
		}
		ports = append(ports, uint16(parsed))
	}
	return ports, nil
}

func effectiveContainerPeer(opts *bpfOptions) string {
	if opts.containerPeer != "" {
		return opts.containerPeer
	}
	return opts.containerIfName + "-p"
}

func writeBPFResult(stdout io.Writer, result bpfResult, output string) error {
	switch output {
	case "json":
		enc := json.NewEncoder(stdout)
		enc.SetIndent("", "  ")
		if err := enc.Encode(result); err != nil {
			return errors.Wrap(err, "cli.writeBPFResult encode json")
		}
	case "text":
		if _, err := fmt.Fprintf(
			stdout,
			"operation: %s\ncontainer: %s\nruntime: %s\ncontainerNetNS: %s\nsandboxNetNS: %s\ncontainerIfName: %s\ncontainerPeer: %s\nsandboxIfName: %s\nproxyMark: 0x%x\n",
			result.Operation,
			result.Container,
			result.Runtime,
			result.ContainerNetNS,
			result.SandboxNetNS,
			result.ContainerIfName,
			result.ContainerPeer,
			result.SandboxIfName,
			result.ProxyMark,
		); err != nil {
			return errors.Wrap(err, "cli.writeBPFResult write text")
		}
		if result.TargetIP != "" {
			if _, err := fmt.Fprintf(stdout, "targetIP: %s\n", result.TargetIP); err != nil {
				return errors.Wrap(err, "cli.writeBPFResult write target ip")
			}
		}
		if len(result.ProxyPorts) > 0 {
			if _, err := fmt.Fprintf(stdout, "proxyPorts: %v\n", result.ProxyPorts); err != nil {
				return errors.Wrap(err, "cli.writeBPFResult write proxy ports")
			}
		}
	default:
		return errors.Errorf("cli.writeBPFResult: unsupported output %q", output)
	}
	return nil
}
