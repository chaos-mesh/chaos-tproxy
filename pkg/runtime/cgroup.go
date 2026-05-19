package runtime

import (
	"bufio"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/pkg/errors"
)

const cgroupV2Controllers = "/sys/fs/cgroup/cgroup.controllers"

// DefaultCgroupManager returns a CgroupManager that dispatches to v1 or v2
// logic based on the host's cgroup hierarchy.
func DefaultCgroupManager() CgroupManager {
	if isCgroupV2() {
		return cgroupV2Manager{}
	}
	return cgroupV1Manager{}
}

// cgroupV2Manager implements CgroupManager for unified cgroup v2.
type cgroupV2Manager struct{}

func (cgroupV2Manager) Join(pid int, info *ContainerInfo) error {
	procsFile := filepath.Join(info.CgroupPath, "cgroup.procs")
	if err := writePid(procsFile, pid); err != nil {
		return errors.Wrapf(err, "runtime.cgroupV2Manager.Join write %s", procsFile)
	}
	return nil
}

// cgroupV1Manager implements CgroupManager for legacy cgroup v1.
// It writes pid into every subsystem hierarchy the container belongs to.
type cgroupV1Manager struct{}

func (cgroupV1Manager) Join(pid int, info *ContainerInfo) error {
	paths, err := cgroupV1Paths(info.PID)
	if err != nil {
		return err
	}
	for subsys, path := range paths {
		procsFile := filepath.Join("/sys/fs/cgroup", subsys, path, "cgroup.procs")
		if err := writePid(procsFile, pid); err != nil {
			return errors.Wrapf(err, "runtime.cgroupV1Manager.Join subsystem %s", subsys)
		}
	}
	return nil
}

func isCgroupV2() bool {
	_, err := os.Stat(cgroupV2Controllers)
	return err == nil
}

// cgroupV1Paths parses /proc/<pid>/cgroup and returns a map of
// subsystem name → cgroup path for all v1 hierarchies.
func cgroupV1Paths(pid uint32) (map[string]string, error) {
	path := fmt.Sprintf("/proc/%d/cgroup", pid)
	f, err := os.Open(path)
	if err != nil {
		return nil, errors.Wrapf(err, "runtime.cgroupV1Paths open %s", path)
	}
	defer f.Close()

	result := make(map[string]string)
	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		// format: <id>:<subsystems>:<path>
		parts := strings.SplitN(scanner.Text(), ":", 3)
		if len(parts) != 3 || parts[0] == "0" {
			continue // skip v2 unified entry
		}
		for _, subsys := range strings.Split(parts[1], ",") {
			if subsys != "" {
				result[subsys] = parts[2]
			}
		}
	}
	if err := scanner.Err(); err != nil {
		return nil, errors.Wrapf(err, "runtime.cgroupV1Paths scan %s", path)
	}
	return result, nil
}

func writePid(procsFile string, pid int) error {
	return os.WriteFile(procsFile, []byte(strconv.Itoa(pid)), 0644)
}
