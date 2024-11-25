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
	// 创建测试用的 Monitor 实例
	t.Log("测试 Monitor")
	m := &Monitor{
		device:  "lo", // 使用本地回环接口进行测试
		timeout: time.Second * 5,
		Key: UDPPacketWithKey{
			Key: "test-key",
		},
		logger: &logger,
	}

	// 设置本地监听地址
	laddr := &net.UDPAddr{
		IP:   net.ParseIP("127.0.0.1"),
		Port: 12346,
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	// 启动监控
	doneChan, err := m.Monitor(ctx, laddr, "test-key")
	if err != nil {
		t.Skip("无法打开网络设备进行测试，可能需要root权限：", err)
		return
	}

	// 创建一个UDP连接来发送测试数据
	go func() {
		time.Sleep(time.Second) // 等待监控启动
		conn, err := net.DialUDP("udp", nil, laddr)
		if err != nil {
			t.Error("创建UDP连接失败:", err)
			return
		}
		defer conn.Close()

		// 发送测试数据
		_, err = conn.Write([]byte(`{"key":"test-key"}`))
		if err != nil {
			t.Error("发送数据失败:", err)
		}
	}()

	// 等待结果
	select {
	case result := <-doneChan:
		assert.True(t, result, "应该收到正确的密钥")

	case <-time.After(3 * time.Second):
		t.Error("测试超时")
	}
}
