package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"time"

	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"
	"github.com/spf13/cobra"
	"gopkg.in/yaml.v3"

	"github.com/chaos-mesh/chaos-tproxy/pkg/config"
	"github.com/chaos-mesh/chaos-tproxy/pkg/runtime"
)

var (
	configFile string
	console    bool
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
	rootCmd.Flags().BoolVarP(&console, "console", "l", false, "use console output")
	rootCmd.Flags().CountVarP(&verbose, "verbose", "v", "verbose level (-v, -vv, -vvv)")
}

func run(cmd *cobra.Command, args []string) error {
	if console {
		log.Logger = log.Output(zerolog.ConsoleWriter{Out: os.Stderr})
	}
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

	if cfg.Target != nil && cfg.Target.Runtime == config.Docker {
		c, err := runtime.NewDockerClient(runtime.WithTimeout(10 * time.Second))
		if err != nil {
			return fmt.Errorf("create docker client: %w", err)
		}
		log.Info().Msg("docker client created")
		info, err := c.ContainerInfo(context.Background(), cfg.Target.Container)
		if err != nil {
			return fmt.Errorf("get container info: %w", err)
		}
		log.Info().Msgf("container info: %+v", info)
	}
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
