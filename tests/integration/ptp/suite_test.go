package ptp_test

import (
	"context"
	"fmt"
	"testing"
	"time"

	"github.com/docker/docker/api/types/container"
	dockerclient "github.com/docker/docker/client"
	dockererrdefs "github.com/docker/docker/errdefs"
	"github.com/pkg/errors"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
)

const ptpDockerImage = "nginx:latest"

var (
	ptpDockerAPI      *dockerclient.Client
	ptpContainerName  string
	ptpContainerNetNS string
)

func TestIntegration(t *testing.T) {
	RegisterFailHandler(Fail)
	RunSpecs(t, "PTP Integration Suite")
}

var _ = BeforeSuite(func() {
	ctx, cancel := context.WithTimeout(context.Background(), 45*time.Second)
	defer cancel()

	var err error
	ptpDockerAPI, err = dockerClient()
	Expect(err).NotTo(HaveOccurred())
	Expect(requireDocker(ctx, ptpDockerAPI)).To(Succeed())
	Expect(requireDockerImage(ctx, ptpDockerAPI, ptpDockerImage)).To(Succeed())

	ptpContainerName = uniqueName("chaos-tproxy-it-ptp-nginx")

	By("starting an nginx container for the PTP integration suite")
	Expect(runDockerContainer(ctx, ptpDockerAPI, ptpContainerName, ptpDockerImage)).To(Succeed())

	pid, err := dockerContainerPID(ctx, ptpDockerAPI, ptpContainerName)
	Expect(err).NotTo(HaveOccurred())
	Expect(pid).NotTo(BeZero())
	ptpContainerNetNS = fmt.Sprintf("/proc/%d/ns/net", pid)
})

var _ = AfterSuite(func() {
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()

	if ptpDockerAPI == nil {
		return
	}
	if ptpContainerName != "" {
		Expect(removeDockerContainer(ctx, ptpDockerAPI, ptpContainerName)).To(Succeed())
	}
	Expect(ptpDockerAPI.Close()).To(Succeed())
})

func dockerClient() (*dockerclient.Client, error) {
	client, err := dockerclient.NewClientWithOpts(
		dockerclient.FromEnv,
		dockerclient.WithAPIVersionNegotiation(),
	)
	if err != nil {
		return nil, errors.Wrap(err, "ptp_test.dockerClient create client")
	}
	return client, nil
}

func requireDocker(ctx context.Context, client *dockerclient.Client) error {
	if _, err := client.Ping(ctx); err != nil {
		return errors.Wrap(err, "ptp_test.requireDocker ping Docker daemon")
	}
	return nil
}

func requireDockerImage(ctx context.Context, client *dockerclient.Client, image string) error {
	if _, err := client.ImageInspect(ctx, image); err != nil {
		return errors.Wrapf(err, "ptp_test.requireDockerImage inspect %s", image)
	}
	return nil
}

func runDockerContainer(ctx context.Context, client *dockerclient.Client, name, image string) error {
	created, err := client.ContainerCreate(
		ctx,
		&container.Config{
			Image: image,
		},
		nil,
		nil,
		nil,
		name,
	)
	if err != nil {
		return errors.Wrapf(err, "ptp_test.runDockerContainer create %s", name)
	}
	if err := client.ContainerStart(ctx, created.ID, container.StartOptions{}); err != nil {
		return errors.Wrapf(err, "ptp_test.runDockerContainer start %s", name)
	}
	return nil
}

func removeDockerContainer(ctx context.Context, client *dockerclient.Client, name string) error {
	if err := client.ContainerRemove(ctx, name, container.RemoveOptions{Force: true}); err != nil {
		if dockererrdefs.IsNotFound(err) {
			return nil
		}
		return errors.Wrapf(err, "ptp_test.removeDockerContainer remove %s", name)
	}
	return nil
}

func dockerContainerPID(ctx context.Context, client *dockerclient.Client, name string) (int, error) {
	inspect, err := client.ContainerInspect(ctx, name)
	if err != nil {
		return 0, errors.Wrapf(err, "ptp_test.dockerContainerPID inspect %s", name)
	}
	return inspect.State.Pid, nil
}
