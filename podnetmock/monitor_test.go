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
		timeout: time.Second * 1,
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

	ctx, cancel := context.WithTimeout(context.Background(), 300*time.Millisecond)
	defer cancel()

	doneChan, err := m.Monitor(ctx, raddr, "test-key")
	assert.Nil(t, err, "no error")
	go func() {
		err := LoopSendKey(ctx, laddr, raddr, "test-key")
		if err != nil {
			t.Error("failed to send key", err)
			cancel()
		}
	}()

	select {
	case <-ctx.Done():
		t.Error("test timeout")
	case result := <-doneChan:
		assert.True(t, result, "get right key")
	}
}
