package main

import (
	"os"

	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"
	"github.com/spf13/cobra"
)

var (
	console bool
	verbose int
)

var rootCmd = &cobra.Command{
	Use:   "chaos-tproxy [FILE]",
	Short: "chaos-tproxy controller",
	Args:  cobra.MaximumNArgs(1),
	PersistentPreRun: func(cmd *cobra.Command, args []string) {
		log.Logger = log.Output(zerolog.ConsoleWriter{Out: os.Stderr})
		switch verbose {
		case 0:
			zerolog.SetGlobalLevel(zerolog.ErrorLevel)
		case 1:
			zerolog.SetGlobalLevel(zerolog.InfoLevel)
		case 2:
			zerolog.SetGlobalLevel(zerolog.DebugLevel)
		default:
			zerolog.SetGlobalLevel(zerolog.TraceLevel)
		}
	},
	// RunE: ,
}

func init() {
	rootCmd.Flags().BoolVarP(&console, "console", "l", false, "use console output")
	rootCmd.Flags().CountVarP(&verbose, "verbose", "v", "verbose level (-v, -vv, -vvv)")
}

func main() {

}
