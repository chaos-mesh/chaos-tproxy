package runtime

import "github.com/pkg/errors"

const (
	RuntimeDocker = "docker"
)

// NewClient creates a RuntimeClient for the named container runtime.
func NewClient(name string, opts ...Option) (RuntimeClient, error) {
	switch name {
	case "", RuntimeDocker:
		return NewDockerClient(opts...)
	default:
		return nil, errors.Errorf("runtime.NewClient: unsupported runtime %q", name)
	}
}
