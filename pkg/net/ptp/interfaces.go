package ptp

import (
	"context"
	"net/netip"

	"github.com/pkg/errors"
)

var (
	// ErrInvalidConfig is returned when required arguments are empty or contain
	// inconsistent namespace or link settings.
	ErrInvalidConfig = errors.New("invalid ptp config")

	// ErrNotReady is returned by Connector.Check when the observed link state
	// does not match the requested link.
	ErrNotReady = errors.New("ptp connection not ready")

	// ErrAlreadyExists is returned by Add when the sandbox interface already
	// exists and force replacement is disabled.
	ErrAlreadyExists = errors.New("ptp device already exist")
)

// Option configures optional PTP connection behavior.
type Option func(*options)

type options struct {
	containerIfName string
	sandboxIP       netip.Prefix
	sandboxIPSet    bool
	force           bool
}

// WithContainerIfName sets the existing container interface used for route
// context. When omitted, the implementation uses eth0.
func WithContainerIfName(name string) Option {
	return func(o *options) {
		o.containerIfName = name
	}
}

// WithSandboxIP assigns an optional IP prefix to the sandbox peer. It is not
// required for the link itself, but Linux requires a local address before it
// accepts a default route via the container IP.
func WithSandboxIP(ip netip.Prefix) Option {
	return func(o *options) {
		o.sandboxIP = ip
		o.sandboxIPSet = true
	}
}

// WithForce replaces an existing sandbox peer before creating the link.
func WithForce(force bool) Option {
	return func(o *options) {
		o.force = force
	}
}

func defaultOptions() *options {
	return &options{
		containerIfName: defaultContainerIfName,
	}
}

const (
	defaultContainerIfName = "eth0"
	ifNameMaxLen           = 13
)

type linkPeerToPeer struct{}

// NewlinkPeerToPeer returns a stateless PTP connector.
func NewlinkPeerToPeer() LinkPlugin {
	return &linkPeerToPeer{}
}

type LinkPlugin interface {
	Add(ctx context.Context, containerNetNS, sandboxNetNS, sandboxIfName string, opts ...Option) error
	Del(ctx context.Context, containerNetNS, sandboxNetNS, sandboxIfName string) error
}
