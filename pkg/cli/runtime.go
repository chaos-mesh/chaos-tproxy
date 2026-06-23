package cli

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"time"

	"github.com/chaos-mesh/chaos-tproxy/pkg/runtime"
	"github.com/pkg/errors"
	"github.com/spf13/cobra"
)

type runtimeInspectOptions struct {
	runtimeName string
	output      string
	timeout     time.Duration
}

func newRuntimeCommand(streams IOStreams) *cobra.Command {
	cmd := &cobra.Command{
		Use:   "runtime",
		Short: "Inspect container runtime metadata",
	}
	cmd.AddCommand(newRuntimeInspectCommand(streams))
	return cmd
}

func newRuntimeInspectCommand(streams IOStreams) *cobra.Command {
	opts := &runtimeInspectOptions{
		runtimeName: runtime.RuntimeDocker,
		output:      "text",
		timeout:     10 * time.Second,
	}

	cmd := &cobra.Command{
		Use:   "inspect CONTAINER",
		Short: "Inspect runtime metadata for a container",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			return runRuntimeInspect(cmd.Context(), streams.Out, args[0], opts)
		},
	}

	cmd.Flags().StringVar(&opts.runtimeName, "runtime", opts.runtimeName, "container runtime")
	cmd.Flags().StringVarP(&opts.output, "output", "o", opts.output, "output format: text or json")
	cmd.Flags().DurationVar(&opts.timeout, "timeout", opts.timeout, "runtime inspect timeout")
	return cmd
}

func runRuntimeInspect(ctx context.Context, stdout io.Writer, container string, opts *runtimeInspectOptions) error {
	client, err := runtime.NewClient(opts.runtimeName, runtime.WithTimeout(opts.timeout))
	if err != nil {
		return errors.Wrap(err, "cli.runRuntimeInspect create runtime client")
	}

	info, err := client.ContainerInfo(ctx, container)
	if err != nil {
		return errors.Wrapf(err, "cli.runRuntimeInspect inspect container %s", container)
	}

	switch opts.output {
	case "json":
		enc := json.NewEncoder(stdout)
		enc.SetIndent("", "  ")
		if err := enc.Encode(info); err != nil {
			return errors.Wrap(err, "cli.runRuntimeInspect encode json")
		}
	case "text":
		if _, err := fmt.Fprintf(
			stdout,
			"netns: %s\npid: %d\ncgroupPath: %s\ncgroupVersion: %d\n",
			info.NetNS,
			info.PID,
			info.CgroupPath,
			info.CgroupVersion,
		); err != nil {
			return errors.Wrap(err, "cli.runRuntimeInspect write text")
		}
	default:
		return errors.Errorf("cli.runRuntimeInspect: unsupported output %q", opts.output)
	}
	return nil
}
