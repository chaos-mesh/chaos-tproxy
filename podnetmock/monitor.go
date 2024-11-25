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
	timer := time.NewTicker(time.Millisecond * 100)
	defer timer.Stop()
	for {
		select {
		case <-ctx.Done():
			return nil
		case <-timer.C:
			conn, err := net.DialUDP("udp", laddr, raddr)
			if err != nil {
				return err
			}
			defer conn.Close()
			packetWithKey := UDPPacketWithKey{
				Key: key,
			}
			payload, err := json.Marshal(packetWithKey)
			if err != nil {
				return err
			}
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
	handle, err := pcap.OpenLive(m.device, 65536, true, time.Millisecond*5)
	if err != nil {
		return nil, err
	}
	// set BPF filter with dst port and protocol UDP
	err = handle.SetBPFFilter(fmt.Sprintf("udp and dst port %d", raddr.Port))
	if err != nil {
		return nil, err
	}

	packetSource := gopacket.NewPacketSource(handle, handle.LinkType())
	packetChan := packetSource.Packets()
	doneChan := make(chan bool, 1)

	go func() {
		for {
			select {
			case <-ctx.Done():
				doneChan <- false
				return
			case packet := <-packetChan:
				udpPacket := packet.Layer(layers.LayerTypeUDP)
				if udpPacket != nil {
					udp, _ := udpPacket.(*layers.UDP)
					if udp.DstPort == layers.UDPPort(raddr.Port) {
						var packetWithKey UDPPacketWithKey
						err := json.Unmarshal(packet.ApplicationLayer().Payload(), &packetWithKey)
						if err != nil {
							m.logger.Error(err, "unmarshal packet with key error", "payload", packet.ApplicationLayer().Payload())
							continue
						}
						if packetWithKey.Key == m.Key.Key {
							doneChan <- true
							return
						}
					}
				}
			}
		}
	}()
	return doneChan, nil
}
