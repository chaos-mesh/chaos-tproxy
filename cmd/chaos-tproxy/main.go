package main

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"

	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"
	"github.com/spf13/cobra"
	"gopkg.in/yaml.v3"

	"github.com/chaos-mesh/chaos-tproxy/pkg/config"
)

var (
	configFile string
	verbose    int
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
	RunE: run,
}

func init() {
	rootCmd.Flags().StringVarP(&configFile, "config", "c", "", "path to config file (JSON or YAML)")
	rootCmd.Flags().CountVarP(&verbose, "verbose", "v", "verbose level (-v, -vv, -vvv)")
}

func run(cmd *cobra.Command, args []string) error {
	if configFile == "" && len(args) > 0 {
		configFile = args[0]
	}
	if configFile == "" {
		return fmt.Errorf("config file is required, use -h for help")
	}

	cfg, err := loadConfig(configFile)
	if err != nil {
		return fmt.Errorf("load config: %w", err)
	}

	log.Info().Str("config", configFile).Msg("config loaded")
	_ = cfg
	// TODO: start proxy with cfg
	return nil
}

func loadConfig(path string) (*config.ChaosTproxyConfig, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}

	var cfg config.ChaosTproxyConfig
	switch filepath.Ext(path) {
	case ".json":
		err = json.Unmarshal(data, &cfg)
	case ".yaml", ".yml":
		err = yaml.Unmarshal(data, &cfg)
	default:
		return nil, fmt.Errorf("unsupported config extension: %s", filepath.Ext(path))
	}
	if err != nil {
		return nil, err
	}
	return &cfg, nil
}

func main() {
	if err := rootCmd.Execute(); err != nil {
		os.Exit(1)
	}
}
