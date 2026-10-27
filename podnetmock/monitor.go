package podnetmock

import (
	"context"
	"encoding/json"
	"fmt"
	"net"
	"time"

	"github.com/google/gopacket"
	"github.com/google/gopacket/layers"
	"github.com/google/gopacket/pcap"
	"k8s.io/klog/v2"
)

type UDPPacketWithKey struct {
	Key string `json:"key"`
}

// LoopSendKey send key to raddr every 100ms
func LoopSendKey(ctx context.Context, laddr *net.UDPAddr, raddr *net.UDPAddr, key string) error {
	if ctx.Err() != nil {
		return nil
	}
	conn, err := net.DialUDP("udp", laddr, raddr)
	if err != nil {
		return err
	}
	defer conn.Close()
	payload, err := json.Marshal(UDPPacketWithKey{Key: key})
	if err != nil {
		return err
	}
	timer := time.NewTicker(time.Millisecond * 100)
	defer timer.Stop()
	for {
		select {
		case <-ctx.Done():
			return nil
		case <-timer.C:
			_, err = conn.Write(payload)
			if err != nil {
				return err
			}
		}
	}
}

type Monitor struct {
	device  string
	timeout time.Duration

	logger *klog.Logger
	Key    UDPPacketWithKey
}

func (m *Monitor) Monitor(ctx context.Context, raddr *net.UDPAddr, key string) (chan bool, error) {
	timeout := m.timeout
	if timeout <= 0 || timeout > 100*time.Millisecond {
		timeout = 100 * time.Millisecond
	}
	handle, err := pcap.OpenLive(m.device, 65536, true, timeout)
	if err != nil {
		return nil, err
	}
	err = handle.SetBPFFilter(fmt.Sprintf("udp and dst port %d", raddr.Port))
	if err != nil {
		handle.Close()
		return nil, err
	}

	packetSource := gopacket.NewPacketSource(handle, handle.LinkType())
	doneChan := make(chan bool, 1)
	go func() {
		matched := false
		defer func() {
			handle.Close()
			doneChan <- matched
		}()
		for {
			if ctx.Err() != nil {
				return
			}
			packet, err := packetSource.NextPacket()
			if ctx.Err() != nil {
				return
			}
			if err == pcap.NextErrorTimeoutExpired {
				continue
			}
			if err != nil {
				return
			}
			udp, ok := packet.Layer(layers.LayerTypeUDP).(*layers.UDP)
			if !ok || udp.DstPort != layers.UDPPort(raddr.Port) {
				continue
			}
			var packetWithKey UDPPacketWithKey
			if err := json.Unmarshal(udp.Payload, &packetWithKey); err != nil {
				m.logger.Error(err, "unmarshal packet with key error", "payload", udp.Payload)
				continue
			}
			if packetWithKey.Key == key {
				matched = true
				return
			}
		}
	}()
	return doneChan, nil
}
