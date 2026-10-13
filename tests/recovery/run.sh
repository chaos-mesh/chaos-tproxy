#!/usr/bin/env bash
set -euo pipefail

test_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
test_container="tproxy-recovery-$(date +%s)-$$"
test_image="$test_container:fixture"
cleanup() {
    docker logs "$test_container" > "$test_dir/$test_container.log" 2>&1 || true
    docker rm -f "$test_container" >/dev/null 2>&1 || true
    docker image rm "$test_image" >/dev/null 2>&1 || true
}
trap cleanup EXIT

docker build --build-arg TPROXY_IMAGE="${1:?pass the tproxy image to test}" -t "$test_image" "$test_dir"
docker run -d --name "$test_container" \
    --cap-add NET_ADMIN --cap-add SYS_ADMIN \
    --security-opt apparmor=unconfined --security-opt seccomp=unconfined \
    --security-opt systempaths=unconfined \
    -p 127.0.0.1::18080 -p 127.0.0.1::8080 "$test_image"
docker exec "$test_container" sh -c \
    'sysctl -w net.ipv6.conf.eth0.disable_ipv6=0 && ip -6 addr add fe80::1234/64 dev eth0'
python3 "$test_dir/check.py" "$test_container"
