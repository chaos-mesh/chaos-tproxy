package ptp

import (
	"net/netip"

	"github.com/pkg/errors"
)

func validateAddArgs(containerNetNS, sandboxNetNS, sandboxIfName string) error {
	if err := validateNetNSPair(containerNetNS, sandboxNetNS); err != nil {
		return err
	}
	if err := validateIfName(sandboxIfName, "sandbox interface name"); err != nil {
		return err
	}
	return nil
}

func validateNetNSPair(containerNetNS, sandboxNetNS string) error {
	if containerNetNS == "" {
		return errors.Wrap(ErrInvalidConfig, "ptp.validate container netns is required")
	}
	if sandboxNetNS == "" {
		return errors.Wrap(ErrInvalidConfig, "ptp.validate sandbox netns is required")
	}
	if containerNetNS == sandboxNetNS {
		return errors.Wrap(ErrInvalidConfig, "ptp.validate container and sandbox netns must differ")
	}
	return nil
}

func validateIfName(name, field string) error {
	if name == "" {
		return errors.Wrapf(ErrInvalidConfig, "ptp.validate %s is required", field)
	}
	if len(name) > ifNameMaxLen {
		return errors.Wrapf(ErrInvalidConfig, "ptp.validate %s %q is longer than %d", field, name, ifNameMaxLen)
	}
	return nil
}

func validateSandboxIP(prefix netip.Prefix) error {
	if !prefix.IsValid() {
		return errors.Wrap(ErrInvalidConfig, "ptp.validate sandbox IP is invalid")
	}
	if !prefix.Addr().Is4() {
		return errors.Wrap(ErrInvalidConfig, "ptp.validate sandbox IP must be an IPv4 prefix")
	}
	return nil
}
