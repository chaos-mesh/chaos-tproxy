package runtime_test

import (
	"context"
	"os"
	"strconv"
	"time"

	"github.com/chaos-mesh/chaos-tproxy/pkg/runtime"
	"github.com/docker/docker/api/types/container"
	"github.com/docker/docker/api/types/strslice"
	dockerclient "github.com/docker/docker/client"
	dockererrdefs "github.com/docker/docker/errdefs"
	"github.com/pkg/errors"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
)

const dockerImage = "busybox:latest"

var _ = Describe("Docker runtime", Label("runtime", "docker"), func() {
	var (
		ctx       context.Context
		cancel    context.CancelFunc
		dockerAPI *dockerclient.Client
	)

	BeforeEach(func() {
		ctx, cancel = context.WithTimeout(context.Background(), 30*time.Second)
		var err error
		dockerAPI, err = dockerClient()
		Expect(err).NotTo(HaveOccurred())
		Expect(requireDocker(ctx, dockerAPI)).To(Succeed())
		Expect(requireDockerImage(ctx, dockerAPI, dockerImage)).To(Succeed())
	})

	AfterEach(func() {
		if dockerAPI != nil {
			Expect(dockerAPI.Close()).To(Succeed())
		}
		cancel()
	})

	Context("when a Docker container is running", func() {
		var containerName string

		BeforeEach(func() {
			containerName = uniqueName("chaos-tproxy-it-runtime")

			By("starting a disposable Docker container")
			Expect(runDockerContainer(ctx, dockerAPI, containerName, dockerImage, "sleep", "300")).To(Succeed())
		})

		AfterEach(func() {
			if containerName == "" || dockerAPI == nil {
				return
			}
			cleanupCtx, cleanupCancel := context.WithTimeout(context.Background(), 30*time.Second)
			defer cleanupCancel()
			Expect(removeDockerContainer(cleanupCtx, dockerAPI, containerName)).To(Succeed())
		})

		When("container info is requested through pkg/runtime", func() {
			It("reports host namespace and cgroup information", func() {
				client, err := runtime.NewDockerClient(runtime.WithTimeout(5 * time.Second))
				Expect(err).NotTo(HaveOccurred())

				info, err := client.ContainerInfo(ctx, containerName)
				Expect(err).NotTo(HaveOccurred())

				expectedPID, err := dockerContainerPID(ctx, dockerAPI, containerName)
				Expect(err).NotTo(HaveOccurred())

				Expect(info.PID).To(Equal(expectedPID))
				Expect(info.NetNS).NotTo(BeEmpty())
				Expect(pathExists(info.NetNS)).To(BeTrue())
				Expect(info.CgroupPath).NotTo(BeEmpty())
				Expect(info.CgroupVersion).To(BeElementOf(runtime.CgroupV1, runtime.CgroupV2))
				if info.CgroupVersion == runtime.CgroupV2 {
					Expect(info.CgroupPath).To(HavePrefix("/sys/fs/cgroup"))
					Expect(pathExists(info.CgroupPath)).To(BeTrue())
				}
			})
		})
	})

})

func setupIntegration() {}

func shutdownIntegration(context.Context) error {
	return nil
}

func dockerClient() (*dockerclient.Client, error) {
	client, err := dockerclient.NewClientWithOpts(
		dockerclient.FromEnv,
		dockerclient.WithAPIVersionNegotiation(),
	)
	if err != nil {
		return nil, errors.Wrap(err, "runtime_test.dockerClient create client")
	}
	return client, nil
}

func requireDocker(ctx context.Context, client *dockerclient.Client) error {
	if _, err := client.Ping(ctx); err != nil {
		return errors.Wrap(err, "runtime_test.requireDocker ping Docker daemon")
	}
	return nil
}

func requireDockerImage(ctx context.Context, client *dockerclient.Client, image string) error {
	if _, err := client.ImageInspect(ctx, image); err != nil {
		return errors.Wrapf(err, "runtime_test.requireDockerImage inspect %s", image)
	}
	return nil
}

func uniqueName(prefix string) string {
	return prefix + "-" + strconv.FormatInt(time.Now().UnixNano(), 36)
}

func runDockerContainer(ctx context.Context, client *dockerclient.Client, name, image string, containerCommand ...string) error {
	created, err := client.ContainerCreate(
		ctx,
		&container.Config{
			Image: image,
			Cmd:   strslice.StrSlice(containerCommand),
		},
		nil,
		nil,
		nil,
		name,
	)
	if err != nil {
		return errors.Wrapf(err, "runtime_test.runDockerContainer create %s", name)
	}
	if err := client.ContainerStart(ctx, created.ID, container.StartOptions{}); err != nil {
		return errors.Wrapf(err, "runtime_test.runDockerContainer start %s", name)
	}
	return nil
}

func removeDockerContainer(ctx context.Context, client *dockerclient.Client, name string) error {
	if err := client.ContainerRemove(ctx, name, container.RemoveOptions{Force: true}); err != nil {
		if dockererrdefs.IsNotFound(err) {
			return nil
		}
		return errors.Wrapf(err, "runtime_test.removeDockerContainer remove %s", name)
	}
	return nil
}

func dockerContainerPID(ctx context.Context, client *dockerclient.Client, name string) (uint32, error) {
	inspect, err := client.ContainerInspect(ctx, name)
	if err != nil {
		return 0, errors.Wrapf(err, "runtime_test.dockerContainerPID inspect %s", name)
	}
	return uint32(inspect.State.Pid), nil
}

func pathExists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}
