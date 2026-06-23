package cli

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/netip"
	"time"

	"github.com/chaos-mesh/chaos-tproxy/pkg/net/ptp"
	containerruntime "github.com/chaos-mesh/chaos-tproxy/pkg/runtime"
	"github.com/pkg/errors"
	"github.com/spf13/cobra"
)

type ptpOptions struct {
	runtimeName     string
	output          string
	timeout         time.Duration
	containerIfName string
	sandboxIP       string
	force           bool
}

type ptpResult struct {
	Operation       string `json:"operation"`
	Container       string `json:"container"`
	Runtime         string `json:"runtime"`
	ContainerNetNS  string `json:"containerNetNS"`
	SandboxNetNS    string `json:"sandboxNetNS"`
	ContainerIfName string `json:"containerIfName,omitempty"`
	SandboxIfName   string `json:"sandboxIfName"`
	SandboxIP       string `json:"sandboxIP,omitempty"`
}

func newPTPCommand(streams IOStreams) *cobra.Command {
	opts := &ptpOptions{
		runtimeName:     containerruntime.RuntimeDocker,
		output:          "text",
		timeout:         10 * time.Second,
		containerIfName: "eth0",
	}

	cmd := &cobra.Command{
		Use:   "ptp",
		Short: "Manage point-to-point netns links",
	}

	cmd.PersistentFlags().StringVar(&opts.runtimeName, "runtime", opts.runtimeName, "container runtime")
	cmd.PersistentFlags().StringVarP(&opts.output, "output", "o", opts.output, "output format: text or json")
	cmd.PersistentFlags().DurationVar(&opts.timeout, "timeout", opts.timeout, "runtime inspect timeout")
	cmd.AddCommand(newPTPAddCommand(streams, opts))
	cmd.AddCommand(newPTPDelCommand(streams, opts))
	return cmd
}

func newPTPAddCommand(streams IOStreams, opts *ptpOptions) *cobra.Command {
	cmd := &cobra.Command{
		Use:   "add CONTAINER SANDBOX_NETNS SANDBOX_IFACE",
		Short: "Create a point-to-point link into a sandbox netns",
		Args:  cobra.ExactArgs(3),
		RunE: func(cmd *cobra.Command, args []string) error {
			return runPTPAdd(cmd.Context(), streams.Out, args[0], args[1], args[2], opts)
		},
	}

	cmd.Flags().StringVar(&opts.containerIfName, "container-iface", opts.containerIfName, "container interface name")
	cmd.Flags().StringVar(&opts.sandboxIP, "sandbox-ip", opts.sandboxIP, "optional sandbox IPv4 prefix")
	cmd.Flags().BoolVar(&opts.force, "force", opts.force, "replace an existing sandbox interface")
	return cmd
}

func newPTPDelCommand(streams IOStreams, opts *ptpOptions) *cobra.Command {
	cmd := &cobra.Command{
		Use:   "del CONTAINER SANDBOX_NETNS SANDBOX_IFACE",
		Short: "Delete a point-to-point link from a sandbox netns",
		Args:  cobra.ExactArgs(3),
		RunE: func(cmd *cobra.Command, args []string) error {
			return runPTPDel(cmd.Context(), streams.Out, args[0], args[1], args[2], opts)
		},
	}

	return cmd
}

func runPTPAdd(ctx context.Context, stdout io.Writer, containerID, sandboxNetNS, sandboxIfName string, opts *ptpOptions) error {
	info, err := resolvePTPContainerInfo(ctx, containerID, opts)
	if err != nil {
		return err
	}

	linkOpts := []ptp.Option{
		ptp.WithContainerIfName(opts.containerIfName),
		ptp.WithForce(opts.force),
	}
	if opts.sandboxIP != "" {
		sandboxIP, err := netip.ParsePrefix(opts.sandboxIP)
		if err != nil {
			return errors.Wrapf(err, "cli.runPTPAdd parse sandbox ip %s", opts.sandboxIP)
		}
		linkOpts = append(linkOpts, ptp.WithSandboxIP(sandboxIP))
	}

	connector := ptp.NewlinkPeerToPeer()
	if err := connector.Add(ctx, info.NetNS, sandboxNetNS, sandboxIfName, linkOpts...); err != nil {
		return errors.Wrapf(err, "cli.runPTPAdd connect container %s", containerID)
	}

	return writePTPResult(stdout, ptpResult{
		Operation:       "add",
		Container:       containerID,
		Runtime:         opts.runtimeName,
		ContainerNetNS:  info.NetNS,
		SandboxNetNS:    sandboxNetNS,
		ContainerIfName: opts.containerIfName,
		SandboxIfName:   sandboxIfName,
		SandboxIP:       opts.sandboxIP,
	}, opts.output)
}

func runPTPDel(ctx context.Context, stdout io.Writer, containerID, sandboxNetNS, sandboxIfName string, opts *ptpOptions) error {
	info, err := resolvePTPContainerInfo(ctx, containerID, opts)
	if err != nil {
		return err
	}

	connector := ptp.NewlinkPeerToPeer()
	if err := connector.Del(ctx, info.NetNS, sandboxNetNS, sandboxIfName); err != nil {
		return errors.Wrapf(err, "cli.runPTPDel disconnect container %s", containerID)
	}

	return writePTPResult(stdout, ptpResult{
		Operation:      "del",
		Container:      containerID,
		Runtime:        opts.runtimeName,
		ContainerNetNS: info.NetNS,
		SandboxNetNS:   sandboxNetNS,
		SandboxIfName:  sandboxIfName,
	}, opts.output)
}

func resolvePTPContainerInfo(ctx context.Context, containerID string, opts *ptpOptions) (*containerruntime.ContainerInfo, error) {
	client, err := containerruntime.NewClient(opts.runtimeName, containerruntime.WithTimeout(opts.timeout))
	if err != nil {
		return nil, errors.Wrap(err, "cli.resolvePTPContainerInfo create runtime client")
	}

	info, err := client.ContainerInfo(ctx, containerID)
	if err != nil {
		return nil, errors.Wrapf(err, "cli.resolvePTPContainerInfo inspect container %s", containerID)
	}
	return info, nil
}

func writePTPResult(stdout io.Writer, result ptpResult, output string) error {
	switch output {
	case "json":
		enc := json.NewEncoder(stdout)
		enc.SetIndent("", "  ")
		if err := enc.Encode(result); err != nil {
			return errors.Wrap(err, "cli.writePTPResult encode json")
		}
	case "text":
		if _, err := fmt.Fprintf(
			stdout,
			"operation: %s\ncontainer: %s\nruntime: %s\ncontainerNetNS: %s\nsandboxNetNS: %s\nsandboxIfName: %s\n",
			result.Operation,
			result.Container,
			result.Runtime,
			result.ContainerNetNS,
			result.SandboxNetNS,
			result.SandboxIfName,
		); err != nil {
			return errors.Wrap(err, "cli.writePTPResult write text")
		}
		if result.ContainerIfName != "" {
			if _, err := fmt.Fprintf(stdout, "containerIfName: %s\n", result.ContainerIfName); err != nil {
				return errors.Wrap(err, "cli.writePTPResult write container interface")
			}
		}
		if result.SandboxIP != "" {
			if _, err := fmt.Fprintf(stdout, "sandboxIP: %s\n", result.SandboxIP); err != nil {
				return errors.Wrap(err, "cli.writePTPResult write sandbox ip")
			}
		}
	default:
		return errors.Errorf("cli.writePTPResult: unsupported output %q", output)
	}
	return nil
}
