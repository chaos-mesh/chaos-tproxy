package runtime

import "context"

// CgroupVersion indicates which cgroup hierarchy the container uses.
type CgroupVersion int

const (
	CgroupV1 CgroupVersion = 1
	CgroupV2 CgroupVersion = 2
)

// ContainerInfo holds the resolved network and cgroup information for an injection target.
type ContainerInfo struct {
	// NetNS is the path to the container's network namespace, e.g.
	// /var/run/docker/netns/<id>. Suitable for nsenter(1) / setns(2).
	NetNS string

	// PID is the container's init process PID on the host. Zero when unavailable.
	PID uint32

	// CgroupPath is the absolute path to the container's cgroup on the host,
	// e.g. /sys/fs/cgroup/system.slice/docker-<id>.scope (cgroup v2).
	// Parsed from /proc/<pid>/cgroup — runtime-agnostic and v1/v2-safe.
	CgroupPath string

	// CgroupVersion is the cgroup hierarchy version used by this container.
	CgroupVersion CgroupVersion
}

// RuntimeClient resolves container metadata needed to inject chaos into a target.
type RuntimeClient interface {
	// ContainerInfo returns network and cgroup information for the given container.
	// containerID may be a full ID or an unambiguous prefix/name.
	ContainerInfo(ctx context.Context, containerID string) (*ContainerInfo, error)
}

// CgroupManager moves a process into a container's cgroup.
type CgroupManager interface {
	// Join moves pid into the cgroup described by info.
	Join(pid int, info *ContainerInfo) error
}
