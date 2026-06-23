package cli

import (
	"context"
	"fmt"
	"io"

	"github.com/pkg/errors"
	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"
	"github.com/spf13/cobra"
)

type IOStreams struct {
	In     io.Reader
	Out    io.Writer
	ErrOut io.Writer
}

type rootOptions struct {
	console bool
	verbose int
}

func NewRootCommand(streams IOStreams) *cobra.Command {
	opts := &rootOptions{}

	cmd := &cobra.Command{
		Use:           "chaos-tproxy [command]",
		Short:         "chaos-tproxy controller",
		SilenceErrors: true,
		SilenceUsage:  true,
		PersistentPreRun: func(cmd *cobra.Command, args []string) {
			configureLogger(streams.ErrOut, opts)
		},
	}

	cmd.PersistentFlags().BoolVarP(&opts.console, "console", "l", false, "use console log output")
	cmd.PersistentFlags().CountVarP(&opts.verbose, "verbose", "v", "verbose level (-v, -vv, -vvv)")
	cmd.AddCommand(newRuntimeCommand(streams))
	cmd.AddCommand(newPTPCommand(streams))
	cmd.AddCommand(newBPFCommand(streams))
	return cmd
}

func Execute(ctx context.Context, args []string, stdin io.Reader, stdout, stderr io.Writer) error {
	streams := IOStreams{
		In:     stdin,
		Out:    stdout,
		ErrOut: stderr,
	}
	cmd := NewRootCommand(streams)
	cmd.SetArgs(args)
	cmd.SetIn(stdin)
	cmd.SetOut(stdout)
	cmd.SetErr(stderr)
	if err := cmd.ExecuteContext(ctx); err != nil {
		return errors.Wrap(err, "cli.Execute execute")
	}
	return nil
}

func Run(ctx context.Context, args []string, stdin io.Reader, stdout, stderr io.Writer) int {
	if err := Execute(ctx, args, stdin, stdout, stderr); err != nil {
		fmt.Fprintf(stderr, "error: %v\n", err)
		return 1
	}
	return 0
}

func configureLogger(stderr io.Writer, opts *rootOptions) {
	if opts.console {
		log.Logger = log.Output(zerolog.ConsoleWriter{Out: stderr})
	} else {
		log.Logger = zerolog.New(stderr).With().Timestamp().Logger()
	}

	switch opts.verbose {
	case 0:
		zerolog.SetGlobalLevel(zerolog.ErrorLevel)
	case 1:
		zerolog.SetGlobalLevel(zerolog.InfoLevel)
	case 2:
		zerolog.SetGlobalLevel(zerolog.DebugLevel)
	default:
		zerolog.SetGlobalLevel(zerolog.TraceLevel)
	}
}
