sudo ip rule add fwmark 1 table 100
sudo ip route add local 0.0.0.0/0 dev lo table 100

sudo iptables -t mangle -N DIVERT
sudo iptables -t mangle -A PREROUTING -p tcp -m socket -j DIVERT
sudo iptables -t mangle -A DIVERT -j MARK --set-mark 1
sudo iptables -t mangle -A DIVERT -j ACCEPT

sudo iptables -t mangle -N CHAOS_PROXY_PREROUTING
sudo iptables -t mangle -A CHAOS_PROXY_PREROUTING -j RETURN -m mark --mark 0xff
sudo iptables -t mangle -A CHAOS_PROXY_PREROUTING -p tcp -j TPROXY --on-port 58080 --tproxy-mark 1
sudo iptables -t mangle -A PREROUTING -p tcp --dport 30000:65535 -j CHAOS_PROXY_PREROUTING

sudo iptables -t mangle -N CHAOS_PROXY_OUTPUT
sudo iptables -t mangle -A CHAOS_PROXY_OUTPUT -j RETURN -m mark --mark 0xff
sudo iptables -t mangle -A CHAOS_PROXY_OUTPUT -p tcp -j MARK --set-mark 1
sudo iptables -t mangle -A OUTPUT -p tcp --sport 30000:65535 -j CHAOS_PROXY_OUTPUT