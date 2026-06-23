.DEFAULT_GOAL := help
SHELL := /bin/bash

COMMIT_SHA := $(shell git rev-parse --short HEAD 2>/dev/null || echo unknown)
VERSION    := $(shell git describe --tags --exact-match 2>/dev/null || echo unpublished)
BUILD_TIME := $(shell date +%Y-%m-%dT%H:%M:%S%z)
LDFLAGS    := -X=main.Commit=$(COMMIT_SHA) -X=main.Version=$(VERSION) -X=main.BuildTime=$(BUILD_TIME)

CHAOS_TPROXY_BIN := ./bin/chaos-tproxy

GO         := CGO_ENABLED=0 go
GCFLAGS    := -gcflags "all=-N -l"
GINKGO     := $(GO) tool github.com/onsi/ginkgo/v2/ginkgo

##@ 帮助
.PHONY: help
help: ## 显示帮助信息
	@awk 'BEGIN {FS = ":.*##"; printf "\n用法:\n  make \033[36m<目标>\033[0m\n"} /^[a-zA-Z0-9_-]+:.*?##/ { printf "  \033[36m%-24s\033[0m %s\n", $$1, $$2 } /^##@/ { printf "\n\033[1m%s\033[0m\n", substr($$0, 5) } ' $(MAKEFILE_LIST)

##@ 运行
.PHONY: chaos-tproxy-run
chaos-tproxy-run: chaos-tproxy-build ## 运行 chaos-tproxy
	$(CHAOS_TPROXY_BIN)

##@ 测试
.PHONY: test-integration
test-integration: test-runtime test-ptp ## 运行全部集成测试

.PHONY: test-runtime
test-runtime: ## 运行 pkg/runtime 集成测试
	$(GINKGO) -r ./tests/integration/runtime

.PHONY: test-ptp
test-ptp: ## 运行 pkg/net/ptp 集成测试
	$(GINKGO) -r ./tests/integration/ptp

##@ 构建
.PHONY: chaos-tproxy-build
chaos-tproxy-build: ## 构建 chaos-tproxy
	$(GO) build $(GOFLAGS) $(GCFLAGS) -ldflags "$(LDFLAGS)" -o $(CHAOS_TPROXY_BIN) ./cmd/chaos-tproxy

##@ 清理
.PHONY: clean
clean: ## 清理构建产物
	rm -f $(CHAOS_TPROXY_BIN)
