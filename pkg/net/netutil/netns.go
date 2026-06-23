package netutil

import (
	"context"
	"fmt"
	"runtime"

	"github.com/vishvananda/netns"
)

// WithNetNS runs fn while the current OS thread is switched to nsPath.
func WithNetNS(ctx context.Context, nsPath string, fn func() error) (err error) {
	if err := ctx.Err(); err != nil {
		return err
	}
	if nsPath == "" {
		return fn()
	}

	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	original, err := netns.Get()
	if err != nil {
		return fmt.Errorf("get current netns: %w", err)
	}
	defer func() { _ = original.Close() }()

	target, err := netns.GetFromPath(nsPath)
	if err != nil {
		return fmt.Errorf("open netns %s: %w", nsPath, err)
	}
	defer func() { _ = target.Close() }()

	if err := netns.Set(target); err != nil {
		return fmt.Errorf("enter netns %s: %w", nsPath, err)
	}
	defer func() {
		if restoreErr := netns.Set(original); restoreErr != nil && err == nil {
			err = fmt.Errorf("restore original netns: %w", restoreErr)
		}
	}()

	return fn()
}
