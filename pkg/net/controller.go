package net

import (
	"github.com/chaos-mesh/chaos-tproxy/pkg/config"
	"github.com/chaos-mesh/chaos-tproxy/pkg/runtime"
	"github.com/rs/zerolog"
)

type ProxyController struct {
	cfg *config.ChaosTproxyConfig

	containerInfo *runtime.ContainerInfo

	cgroupManager runtime.CgroupManager

	logger *zerolog.Logger
}

func NewProxyController(cfg *config.ChaosTproxyConfig, containerInfo *runtime.ContainerInfo, logger *zerolog.Logger) *ProxyController {
	return &ProxyController{
		cfg:           cfg,
		containerInfo: containerInfo,
		cgroupManager: runtime.DefaultCgroupManager(),
		logger:        logger,
	}
}

func (c *ProxyController) Run() error {
	return nil
}

func (c *ProxyController) Stop() error {
	return nil
}
