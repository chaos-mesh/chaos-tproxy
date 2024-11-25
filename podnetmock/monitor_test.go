package podnetmock

import (
	"context"
	"net"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"k8s.io/klog/v2"
)

func TestMonitor(t *testing.T) {
	logger := klog.NewKlogr().WithName("test-monitor")
	m := &Monitor{
		device:  "lo",
		timeout: time.Second * 5,
		Key: UDPPacketWithKey{
			Key: "test-key",
		},
		logger: &logger,
	}

	laddr := &net.UDPAddr{
		IP:   net.ParseIP("127.0.0.1"),
		Port: 12346,
	}

	raddr := &net.UDPAddr{
		IP:   net.ParseIP("127.0.0.1"),
		Port: 12345,
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	doneChan, err := m.Monitor(ctx, raddr, "test-key")
	if err != nil {
		t.Skip("failed to monitor", err)
		return
	}

	go func() {
		LoopSendKey(ctx, laddr, raddr, "test-key")
	}()

	select {
	case result := <-doneChan:
		assert.True(t, result, "get right key")

	case <-time.After(300 * time.Second):
		t.Error("test timeout")
	}
}
