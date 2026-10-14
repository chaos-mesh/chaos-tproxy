package podnetmock

import (
	"context"
	"net"
	"os"
	"runtime"
	"strings"
	"testing"
	"time"

	"github.com/google/gopacket/pcap"
	"k8s.io/klog/v2"
)

func listenUDP(t *testing.T) *net.UDPConn {
	t.Helper()
	conn, err := net.ListenUDP("udp4", &net.UDPAddr{IP: net.IPv4(127, 0, 0, 1)})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { conn.Close() })
	return conn
}

func TestLoopSendKey(t *testing.T) {
	receiver := listenUDP(t)
	source := listenUDP(t)
	laddr := source.LocalAddr().(*net.UDPAddr)
	source.Close()
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	done := make(chan error, 1)
	go func() { done <- LoopSendKey(ctx, laddr, receiver.LocalAddr().(*net.UDPAddr), "test-key") }()
	t.Cleanup(func() {
		cancel()
		select {
		case err := <-done:
			if err != nil {
				t.Errorf("sender returned %v", err)
			}
		case <-time.After(time.Second):
			t.Error("sender did not stop")
		}
		rebound, err := net.ListenUDP("udp4", laddr)
		if err != nil {
			t.Errorf("source port was not released: %v", err)
		} else {
			rebound.Close()
		}
	})
	if err := receiver.SetReadDeadline(time.Now().Add(time.Second)); err != nil {
		t.Fatal(err)
	}
	buf := make([]byte, 128)
	for i := 0; i < 3; i++ {
		n, addr, err := receiver.ReadFromUDP(buf)
		if err != nil {
			t.Fatalf("datagram %d: %v", i+1, err)
		}
		if string(buf[:n]) != `{"key":"test-key"}` {
			t.Fatalf("unexpected datagram %q", buf[:n])
		}
		if addr.Port != laddr.Port {
			t.Fatalf("source port = %d, want %d", addr.Port, laddr.Port)
		}
	}
}

func packetSockets(t *testing.T) int {
	t.Helper()
	data, err := os.ReadFile("/proc/net/packet")
	if err != nil {
		t.Fatal(err)
	}
	return len(strings.Split(strings.TrimSpace(string(data)), "\n")) - 1
}

func newMonitor(timeout time.Duration) *Monitor {
	logger := klog.NewKlogr().WithName("test-monitor")
	return &Monitor{device: "lo", timeout: timeout, Key: UDPPacketWithKey{Key: "field-key"}, logger: &logger}
}

func assertCaptureReleased(t *testing.T, sockets, goroutines int) {
	t.Helper()
	if got := packetSockets(t); got != sockets {
		t.Errorf("packet sockets after completion = %d, want %d", got, sockets)
	}
	deadline := time.Now().Add(time.Second)
	for runtime.NumGoroutine() > goroutines && time.Now().Before(deadline) {
		time.Sleep(10 * time.Millisecond)
	}
	if got := runtime.NumGoroutine(); got > goroutines {
		t.Errorf("goroutines after completion = %d, baseline %d", got, goroutines)
	}
}

func TestMonitor(t *testing.T) {
	for _, tc := range []struct {
		name    string
		key     string
		payload string
	}{
		{name: "argument key", key: "argument-key", payload: `{"key":"argument-key"}`},
		{name: "empty key", key: "", payload: `{"key":""}`},
	} {
		t.Run(tc.name, func(t *testing.T) {
			for i := 0; i < 3; i++ {
				receiver := listenUDP(t)
				sockets, goroutines := packetSockets(t), runtime.NumGoroutine()
				ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
				defer cancel()
				done, err := newMonitor(20*time.Millisecond).Monitor(ctx, receiver.LocalAddr().(*net.UDPAddr), tc.key)
				if err != nil {
					t.Fatal(err)
				}
				if got := packetSockets(t); got != sockets+1 {
					t.Fatalf("open packet sockets = %d, want %d", got, sockets+1)
				}
				sender, err := net.DialUDP("udp4", nil, receiver.LocalAddr().(*net.UDPAddr))
				if err != nil {
					t.Fatal(err)
				}
				t.Cleanup(func() { sender.Close() })
				ticker := time.NewTicker(20 * time.Millisecond)
				start := time.Now()
				guard := time.NewTimer(3 * time.Second)
				defer guard.Stop()
			waiting:
				for {
					select {
					case result := <-done:
						if !result {
							t.Error("argument key did not match")
						}
						break waiting
					case <-guard.C:
						t.Fatal("monitor did not complete")
					case <-ticker.C:
						payload := []byte(tc.payload)
						if time.Since(start) < 100*time.Millisecond {
							payload = nil
						}
						if _, err := sender.Write(payload); err != nil {
							t.Fatal(err)
						}
					}
				}
				ticker.Stop()
				cancel()
				sender.Close()
				receiver.Close()
				assertCaptureReleased(t, sockets, goroutines)
			}
		})
	}
}

func TestMonitorCancellation(t *testing.T) {
	for _, timeout := range []time.Duration{20 * time.Millisecond, 500 * time.Microsecond, 0, pcap.BlockForever, time.Hour} {
		t.Run(timeout.String(), func(t *testing.T) {
			for i := 0; i < 3; i++ {
				receiver := listenUDP(t)
				sockets, goroutines := packetSockets(t), runtime.NumGoroutine()
				ctx, cancel := context.WithCancel(context.Background())
				defer cancel()
				done, err := newMonitor(timeout).Monitor(ctx, receiver.LocalAddr().(*net.UDPAddr), "argument-key")
				if err != nil {
					t.Fatal(err)
				}
				sender, err := net.DialUDP("udp4", nil, receiver.LocalAddr().(*net.UDPAddr))
				if err != nil {
					t.Fatal(err)
				}
				if _, err := sender.Write([]byte(`{"key":"field-key"}`)); err != nil {
					t.Fatal(err)
				}
				sender.Close()
				time.Sleep(150 * time.Millisecond)
				select {
				case result := <-done:
					t.Fatalf("field key completed monitor with %v", result)
				default:
				}
				cancel()
				select {
				case result := <-done:
					if result {
						t.Error("cancellation returned true")
					}
				case <-time.After(time.Second):
					t.Fatal("cancellation did not stop idle capture")
				}
				receiver.Close()
				assertCaptureReleased(t, sockets, goroutines)
			}
		})
	}
}

func TestMonitorFilterFailure(t *testing.T) {
	sockets, goroutines := packetSockets(t), runtime.NumGoroutine()
	done, err := newMonitor(20*time.Millisecond).Monitor(context.Background(), &net.UDPAddr{Port: 65536}, "argument-key")
	if err == nil {
		t.Fatal("out-of-range filter port was accepted")
	}
	if done != nil {
		t.Fatal("filter failure returned a completion channel")
	}
	assertCaptureReleased(t, sockets, goroutines)
}
