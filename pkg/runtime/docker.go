package runtime

import (
	"bufio"
	"context"
	"fmt"
	"os"
	"strings"

	dockerclient "github.com/docker/docker/client"
	"github.com/pkg/errors"
)

// DockerClient implements RuntimeClient against a local Docker daemon.
type DockerClient struct {
	inner *dockerclient.Client
	opts  *options
}

// NewDockerClient creates a DockerClient.
// The Docker socket is resolved from the environment (DOCKER_HOST) or the
// default /var/run/docker.sock, matching the behaviour of `docker` CLI.
func NewDockerClient(opts ...Option) (*DockerClient, error) {
	o := defaultOptions()
	for _, opt := range opts {
		opt(o)
	}

	inner, err := dockerclient.NewClientWithOpts(
		dockerclient.FromEnv,
		dockerclient.WithAPIVersionNegotiation(),
	)
	if err != nil {
		return nil, errors.Wrap(err, "runtime.NewDockerClient create client")
	}
	return &DockerClient{inner: inner, opts: o}, nil
}

// ContainerInfo returns the netns path, host PID, and cgroup path for the given container.
// Docker exposes the netns as SandboxKey, which is the path passed to setns(2).
// The cgroup path is read from /proc/<pid>/cgroup and is v1/v2-safe.
func (c *DockerClient) ContainerInfo(ctx context.Context, containerID string) (*ContainerInfo, error) {
	ctx, cancel := context.WithTimeout(ctx, c.opts.timeout)
	defer cancel()

	info, err := c.inner.ContainerInspect(ctx, containerID)
	if err != nil {
		return nil, errors.Wrapf(err, "runtime.DockerClient.ContainerInfo inspect %s", containerID)
	}

	if info.NetworkSettings == nil {
		return nil, errors.New(fmt.Sprintf("runtime.DockerClient.ContainerInfo: container %s has no network settings", containerID))
	}

	netns := info.NetworkSettings.SandboxKey
	if netns == "" {
		return nil, errors.New(fmt.Sprintf("runtime.DockerClient.ContainerInfo: container %s has empty SandboxKey", containerID))
	}

	pid := uint32(info.State.Pid)

	cgroupPath, err := readCgroupPath(pid)
	if err != nil {
		return nil, err
	}

	cgroupVer := CgroupV1
	if isCgroupV2() {
		cgroupVer = CgroupV2
	}

	return &ContainerInfo{
		NetNS:         netns,
		PID:           pid,
		CgroupPath:    cgroupPath,
		CgroupVersion: cgroupVer,
	}, nil
}

// readCgroupPath reads /proc/<pid>/cgroup and returns the cgroup v2 unified path,
// falling back to the first entry for cgroup v1 hierarchies.
//
// cgroup v2 line format:  0::<path>
// cgroup v1 line format:  <id>:<subsystems>:<path>
func readCgroupPath(pid uint32) (string, error) {
	path := fmt.Sprintf("/proc/%d/cgroup", pid)
	f, err := os.Open(path)
	if err != nil {
		return "", errors.Wrapf(err, "runtime.readCgroupPath open %s", path)
	}
	defer f.Close()

	var first string
	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		parts := strings.SplitN(scanner.Text(), ":", 3)
		if len(parts) != 3 {
			continue
		}
		// cgroup v2: hierarchy id is "0", subsystems field is empty
		if parts[0] == "0" {
			return "/sys/fs/cgroup" + parts[2], nil
		}
		if first == "" {
			first = parts[2]
		}
	}
	if err := scanner.Err(); err != nil {
		return "", errors.Wrapf(err, "runtime.readCgroupPath scan %s", path)
	}
	if first == "" {
		return "", errors.Errorf("runtime.readCgroupPath: no cgroup entry found in %s", path)
	}
	return first, nil
}
