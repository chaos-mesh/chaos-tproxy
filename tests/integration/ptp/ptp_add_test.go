package ptp_test

import (
	"context"
	stderrors "errors"
	"net/netip"
	"os"
	"path/filepath"
	"runtime"
	"strconv"
	"time"

	"github.com/chaos-mesh/chaos-tproxy/pkg/net/ptp"
	"github.com/pkg/errors"
	"github.com/vishvananda/netlink"
	"github.com/vishvananda/netns"

	. "github.com/onsi/ginkgo/v2"
	. "github.com/onsi/gomega"
)

const sandboxIfName = "ptp0"

var _ = Describe("PTP link", Label("ptp", "docker"), func() {
	var (
		ctx          context.Context
		cancel       context.CancelFunc
		sandboxName  string
		sandboxNetNS string
		connector    ptp.LinkPlugin
	)

	BeforeEach(func() {
		ctx, cancel = context.WithTimeout(context.Background(), 30*time.Second)
		sandboxName = uniqueName("chaos-tproxy-it-ptp")
		sandboxNetNS = filepath.Join("/run/netns", sandboxName)
		connector = ptp.NewlinkPeerToPeer()

		Expect(createNamedNetNS(sandboxName)).To(Succeed())
	})

	AfterEach(func() {
		cleanupCtx, cleanupCancel := context.WithTimeout(context.Background(), 30*time.Second)
		defer cleanupCancel()

		if connector != nil && sandboxNetNS != "" {
			Expect(connector.Del(cleanupCtx, ptpContainerNetNS, sandboxNetNS, sandboxIfName)).To(Succeed())
		}
		if sandboxName != "" {
			Expect(deleteNamedNetNS(sandboxName)).To(Succeed())
		}
		cancel()
	})

	Context("when a sandbox netns is empty", func() {
		When("a PTP link is added more than once", func() {
			It("creates the first link, rejects duplicates, and replaces it with force", func() {
				firstIP := netip.MustParsePrefix("169.254.200.2/32")
				Expect(connector.Add(
					ctx,
					ptpContainerNetNS,
					sandboxNetNS,
					sandboxIfName,
					ptp.WithSandboxIP(firstIP),
				)).To(Succeed())
				Expect(sandboxLinkAddr(sandboxNetNS, sandboxIfName)).To(Equal(firstIP.Addr()))

				err := connector.Add(
					ctx,
					ptpContainerNetNS,
					sandboxNetNS,
					sandboxIfName,
					ptp.WithSandboxIP(firstIP),
				)
				Expect(stderrors.Is(err, ptp.ErrAlreadyExists)).To(BeTrue())

				forcedIP := netip.MustParsePrefix("169.254.200.3/32")
				Expect(connector.Add(
					ctx,
					ptpContainerNetNS,
					sandboxNetNS,
					sandboxIfName,
					ptp.WithSandboxIP(forcedIP),
					ptp.WithForce(true),
				)).To(Succeed())
				Expect(sandboxLinkAddr(sandboxNetNS, sandboxIfName)).To(Equal(forcedIP.Addr()))
			})
		})
	})
})

func createNamedNetNS(name string) (err error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	original, err := netns.Get()
	if err != nil {
		return errors.Wrap(err, "ptp_test.createNamedNetNS get original netns")
	}
	defer original.Close()
	defer func() {
		if restoreErr := netns.Set(original); restoreErr != nil && err == nil {
			err = errors.Wrap(restoreErr, "ptp_test.createNamedNetNS restore original netns")
		}
	}()

	created, err := netns.NewNamed(name)
	if err != nil {
		return errors.Wrapf(err, "ptp_test.createNamedNetNS create %s", name)
	}
	return created.Close()
}

func deleteNamedNetNS(name string) error {
	if err := netns.DeleteNamed(name); err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		return errors.Wrapf(err, "ptp_test.deleteNamedNetNS delete %s", name)
	}
	return nil
}

func sandboxLinkAddr(nsPath, ifName string) (netip.Addr, error) {
	var got netip.Addr
	err := withNetNS(nsPath, func() error {
		link, err := netlink.LinkByName(ifName)
		if err != nil {
			return errors.Wrapf(err, "ptp_test.sandboxLinkAddr look up %s", ifName)
		}
		addrs, err := netlink.AddrList(link, netlink.FAMILY_V4)
		if err != nil {
			return errors.Wrapf(err, "ptp_test.sandboxLinkAddr list addresses on %s", ifName)
		}
		for _, addr := range addrs {
			parsed, ok := netip.AddrFromSlice(addr.IP)
			if ok {
				got = parsed.Unmap()
				return nil
			}
		}
		return errors.Errorf("ptp_test.sandboxLinkAddr no IPv4 address on %s", ifName)
	})
	return got, err
}

func withNetNS(nsPath string, fn func() error) (err error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	original, err := netns.Get()
	if err != nil {
		return errors.Wrap(err, "ptp_test.withNetNS get original netns")
	}
	defer original.Close()

	target, err := netns.GetFromPath(nsPath)
	if err != nil {
		return errors.Wrapf(err, "ptp_test.withNetNS open %s", nsPath)
	}
	defer target.Close()

	if err := netns.Set(target); err != nil {
		return errors.Wrapf(err, "ptp_test.withNetNS enter %s", nsPath)
	}
	defer func() {
		if restoreErr := netns.Set(original); restoreErr != nil && err == nil {
			err = errors.Wrap(restoreErr, "ptp_test.withNetNS restore original netns")
		}
	}()

	return fn()
}

func uniqueName(prefix string) string {
	return prefix + "-" + strconv.FormatInt(time.Now().UnixNano(), 36)
}
