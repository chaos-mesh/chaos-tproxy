package ptp

import (
	stderrors "errors"
	"os"
	"runtime"

	"github.com/pkg/errors"
	"github.com/vishvananda/netlink"
	"github.com/vishvananda/netns"
)

func withNetNS(nsPath string, fn func() error) (err error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	original, err := netns.Get()
	if err != nil {
		return errors.Wrap(err, "ptp.withNetNS get original netns")
	}
	defer original.Close()

	target, err := netns.GetFromPath(nsPath)
	if err != nil {
		return errors.Wrapf(err, "ptp.withNetNS open %s", nsPath)
	}
	defer target.Close()

	if err := netns.Set(target); err != nil {
		return errors.Wrapf(err, "ptp.withNetNS enter %s", nsPath)
	}
	defer func() {
		if restoreErr := netns.Set(original); restoreErr != nil && err == nil {
			err = errors.Wrap(restoreErr, "ptp.withNetNS restore original netns")
		}
	}()

	return fn()
}

func isLinkNotFound(err error) bool {
	var notFound netlink.LinkNotFoundError
	return stderrors.As(err, &notFound)
}

func isNetNSNotFound(err error) bool {
	return stderrors.Is(err, os.ErrNotExist)
}
