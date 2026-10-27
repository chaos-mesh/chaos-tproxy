import ipaddress
import json
import os
from pathlib import Path
import select
import shlex
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import time
import unittest
import urllib.request
from urllib.parse import parse_qsl, urlsplit


BINARY = os.environ.get("TPROXY_BINARY", "/tproxy")
NAT_RULE_CHAINS = ("OUTPUT", "PREROUTING")


def run(*args):
    return subprocess.check_output(args, text=True).strip()


def proxy_config(marker, safe_mode=False):
    return {
        "proxy_ports": [18080],
        "safe_mode": safe_mode,
        "rules": [{
            "target": "Request",
            "selector": {"method": "GET"},
            "actions": {"replace": {"headers": {"X-Tproxy-Test": marker}}},
        }],
    }


class LifecycleTests(unittest.TestCase):
    def setUp(self):
        if sys.platform != "linux" or os.geteuid() != 0:
            self.skipTest("requires root inside a disposable Linux container")
        if os.environ.get("CHAOS_TPROXY_TEST_ISOLATED") != "1":
            self.skipTest("set CHAOS_TPROXY_TEST_ISOLATED=1 only in a disposable container")
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.logs = tempfile.TemporaryFile()
        self.addCleanup(self.logs.close)
        self.process = None
        self.addCleanup(self.cleanup)
        self.baseline_addresses = self.addresses()
        self.baseline_default = run("ip", "-4", "route", "show", "default")
        self.baseline_dns = Path("/etc/resolv.conf").read_bytes()
        self.baseline_links = self.links()
        self.baseline_namespaces = run("ip", "netns", "list")
        for chain in NAT_RULE_CHAINS:
            run("ebtables", "-t", "nat", "-A", chain, "-p", "IPv4",
                "--ip-proto", "17", "--ip-dport", "65000", "-j", "ACCEPT")
        self.baseline_nat = run("ebtables", "-t", "nat", "-L")
        self.assert_connectivity()

    def links(self):
        return {item["ifname"] for item in json.loads(run("ip", "-j", "link", "show"))}

    def addresses(self):
        return sorted((item["ifname"], address["local"], address["prefixlen"])
                      for item in json.loads(run("ip", "-j", "-4", "address", "show"))
                      for address in item["addr_info"])

    def start(self, env=None, file_config=None):
        args = [BINARY, "-v"]
        if file_config is None:
            args.append("-i")
        else:
            config_file = Path(self.directory.name) / "config.json"
            config_file.write_text(json.dumps(file_config))
            args.append(str(config_file))
        self.response_buffer = b""
        self.process = subprocess.Popen(args, stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE, stderr=self.logs,
                                        start_new_session=True, env=env)
        return self.process

    def request(self, config, connection_close=False, padding=0):
        body = b" " * padding + json.dumps(config, separators=(",", ":")).encode()
        return (b"PUT / HTTP/1.1\r\nHost: localhost\r\nContent-Length: "
                   + str(len(body)).encode() + (b"\r\nConnection: close\r\n\r\n" if connection_close else b"\r\n\r\n") + body)

    def put(self, config, connection_close=False):
        self.process.stdin.write(self.request(config, connection_close))
        self.process.stdin.flush()
        return self.read_response()

    def read_response(self):
        response = self.response_buffer
        deadline = time.monotonic() + 20
        while b"\r\n\r\n" not in response:
            remaining = deadline - time.monotonic()
            self.assertGreater(remaining, 0, "configuration response timed out")
            ready, _, _ = select.select([self.process.stdout], [], [], remaining)
            if ready:
                chunk = os.read(self.process.stdout.fileno(), 65536)
                self.assertTrue(chunk, "controller closed stdout instead of replying")
                response += chunk
        header, body = response.split(b"\r\n\r\n", 1)
        length = next((int(line.split(b":", 1)[1]) for line in header.split(b"\r\n")[1:]
                       if line.lower().startswith(b"content-length:")), 0)
        while len(body) < length:
            ready, _, _ = select.select([self.process.stdout], [], [], max(0, deadline - time.monotonic()))
            self.assertTrue(ready, "configuration error body timed out")
            chunk = os.read(self.process.stdout.fileno(), 65536)
            self.assertTrue(chunk, "configuration response body was truncated")
            body += chunk
        self.response_buffer = body[length:]
        return int(header.split()[1]), body[:length].decode(errors="replace")

    def gateway_ping_observer(self):
        route = json.loads(run("ip", "-j", "-4", "route", "show", "default"))[0]
        watch = socket.socket(socket.AF_PACKET, socket.SOCK_RAW, socket.htons(0x0003))
        self.addCleanup(watch.close)
        watch.bind((route["dev"], 0))
        watch.setblocking(False)
        return watch, socket.inet_aton(route["gateway"])

    def gateway_pings(self, observer, duration=2.25):
        watch, gateway = observer
        while True:
            try:
                watch.recv(65535)
            except BlockingIOError:
                break
        deadline = time.monotonic() + duration
        packets = []
        while time.monotonic() < deadline:
            if not select.select([watch], [], [], max(0, deadline - time.monotonic()))[0]:
                break
            frame = watch.recv(65535)
            if len(frame) < 42 or frame[12:14] != b"\x08\x00":
                continue
            ip = frame[14:]
            if ip[9] != 1 or ip[16:20] != gateway:
                continue
            icmp = ip[(ip[0] & 15) * 4:]
            if len(icmp) >= 8 and icmp[0] == 8:
                packets.append(struct.unpack("!HH", icmp[4:8]))
        return packets

    def assert_one_gateway_ping(self, observer):
        packets = self.gateway_pings(observer)
        self.assertGreaterEqual(len(packets), 2, "gateway echo requests were not observed")
        self.assertEqual(len({identifier for identifier, _ in packets}), 1, packets)
        sequences = [sequence for _, sequence in packets]
        self.assertTrue(all(current == (previous + 1) % 65536
                            for previous, current in zip(sequences, sequences[1:])), sequences)

    def assert_gateway_ping_reload_and_stop(self, safe_mode):
        observer = self.gateway_ping_observer()
        self.assertEqual(self.gateway_pings(observer, duration=0.1), [])
        self.start()
        for index in range(4):
            marker = "proxy-ping-" + str(index)
            self.assertEqual(self.put(proxy_config(marker, safe_mode=safe_mode))[0], 200)
            self.assert_connectivity(marker)
            self.assert_one_gateway_ping(observer)
        self.assertEqual(self.put({})[0], 200)
        self.assertIsNone(self.process.poll())
        self.assertEqual(self.gateway_pings(observer), [])
        self.assert_restored()

    def test_gateway_ping_reload_and_stop(self):
        self.assert_gateway_ping_reload_and_stop(False)

    def test_gateway_ping_safe_reload_and_stop(self):
        self.assert_gateway_ping_reload_and_stop(True)

    def test_gateway_ping_failed_start_stops(self):
        observer = self.gateway_ping_observer()
        self.start()
        self.assertEqual(self.put(proxy_config("proxy-ping-before-failure"))[0], 200)
        self.assert_one_gateway_ping(observer)
        self.assertEqual(self.put({"proxy_ports": [18080], "rules": [
            {"target": "Request", "selector": {"method": "BAD METHOD"}, "actions": {}}]})[0], 500)
        self.assertIsNone(self.process.poll())
        self.assertEqual(self.gateway_pings(observer), [])
        self.assert_restored()
        self.assertEqual(self.put(proxy_config("proxy-ping-recovered"))[0], 200)
        self.assert_one_gateway_ping(observer)
        self.assertEqual(self.put({})[0], 200)
        self.assertEqual(self.gateway_pings(observer), [])
        self.assert_restored()

    def assert_connectivity(self, expected_marker="direct"):
        url = os.environ.get("TPROXY_TEST_URL")
        self.assertTrue(url, "TPROXY_TEST_URL is required for the HTTP connectivity check")
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        with opener.open(url, timeout=5) as response:
            self.assertEqual(response.status, 200)
            self.assertEqual(response.read(), b"network-ok\n")
            self.assertEqual(response.headers.get("X-Tproxy-Test"), expected_marker)

    def assert_restored(self, expected_default=None):
        self.assertEqual(self.addresses(), self.baseline_addresses)
        if expected_default is None:
            self.assertEqual(run("ip", "-4", "route", "show", "default"), self.baseline_default)
        else:
            defaults = json.loads(run("ip", "-j", "-4", "route", "show", "default"))
            self.assertEqual(len(defaults), 1)
            for key, value in expected_default.items():
                self.assertEqual(defaults[0][key], value)
        self.assertEqual(Path("/etc/resolv.conf").read_bytes(), self.baseline_dns)
        self.assertEqual(self.links(), self.baseline_links)
        self.assertEqual(run("ip", "netns", "list"), self.baseline_namespaces)
        self.assertEqual(run("ebtables", "-t", "nat", "-L"), self.baseline_nat)
        self.assert_connectivity()

    def cleanup(self):
        if self.process is not None:
            if self.process.poll() is None:
                os.killpg(self.process.pid, signal.SIGTERM)
                try:
                    self.process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(self.process.pid, signal.SIGKILL)
                    self.process.wait(timeout=2)
            for stream in (self.process.stdin, self.process.stdout):
                stream.close()
        for chain in NAT_RULE_CHAINS:
            subprocess.run(["ebtables", "-t", "nat", "-D", chain, "-p", "IPv4",
                            "--ip-proto", "17", "--ip-dport", "65000", "-j", "ACCEPT"],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    def test_invalid_method_rolls_back_and_accepts_next_config(self):
        self.start()
        status, _ = self.put({"proxy_ports": [8080], "rules": [
            {"target": "Request", "selector": {"method": "BAD METHOD"}, "actions": {}}]})
        self.assertNotEqual(status, 200, "invalid child configuration was acknowledged")
        self.assert_restored()
        self.assertEqual(self.put({"proxy_ports": [8080]})[0], 200)
        self.assertEqual(self.put({})[0], 200)
        self.assert_restored()

    def test_invalid_tls_rolls_back(self):
        self.start()
        status, _ = self.put({"proxy_ports": [443], "tls": {
            "cert_file": {"type": "Path", "value": "/missing-audit-cert.pem"},
            "key_file": {"type": "Path", "value": "/missing-audit-key.pem"}}})
        self.assertNotEqual(status, 200, "unreadable TLS configuration was acknowledged")
        self.assert_restored()

    def test_child_exec_failure_rolls_back(self):
        wrapper = Path(self.directory.name) / "ip"
        wrapper.write_text("#!/usr/bin/env bash\nif [ \"$1\" = netns ] && [ \"$2\" = exec ] && [ \"$4\" = "
                           + shlex.quote(BINARY) + " ]; then exit 42; fi\nexec /sbin/ip \"$@\"\n")
        wrapper.chmod(0o755)
        env = dict(os.environ, PATH=self.directory.name + ":" + os.environ["PATH"])
        self.start(env=env)
        self.assertNotEqual(self.put({"proxy_ports": [8080]})[0], 200,
                            "failed child command was acknowledged")
        self.assert_restored()

    def test_network_setup_failure_rolls_back(self):
        wrapper = Path(self.directory.name) / "ip"
        wrapper.write_text("#!/usr/bin/env bash\nif [ \"$1\" = route ] && [ \"$2\" = add ] && [ \"$3\" = default ]; "
                           "then exit 42; fi\nexec /sbin/ip \"$@\"\n")
        wrapper.chmod(0o755)
        self.start(env=dict(os.environ, PATH=self.directory.name + ":" + os.environ["PATH"]))
        status, body = self.put({"proxy_ports": [8080]})
        self.assertEqual(status, 500)
        self.assertIn("42", body)
        self.assert_restored()

    def test_listener_bind_failure_rolls_back(self):
        blocker = Path(self.directory.name) / "block_listener.py"
        blocker.write_text("import socket,subprocess,sys\n"
                           "listener=socket.socket()\n"
                           "listener.bind(('0.0.0.0',1025))\n"
                           "listener.listen()\n"
                           "sys.exit(subprocess.call(sys.argv[1:]))\n")
        wrapper = Path(self.directory.name) / "ip"
        wrapper.write_text("#!/usr/bin/env bash\nif [ \"$1\" = netns ] && [ \"$2\" = exec ] && [ \"$4\" = "
                           + shlex.quote(BINARY) + " ]; then namespace=\"$3\"; shift 3; exec /sbin/ip netns exec \"$namespace\" python3 "
                           + shlex.quote(str(blocker)) + " \"$@\"; fi\nexec /sbin/ip \"$@\"\n")
        wrapper.chmod(0o755)
        self.start(env=dict(os.environ, PATH=self.directory.name + ":" + os.environ["PATH"]))
        status, body = self.put({"proxy_ports": [8080]})
        self.assertNotEqual(status, 200, "listener failure was acknowledged")
        self.assertIn("Address already in use", body)
        self.assert_restored()

    def test_readiness_timeout_rolls_back(self):
        wrapper = Path(self.directory.name) / "ip"
        wrapper.write_text("#!/usr/bin/env bash\nif [ \"$1\" = netns ] && [ \"$2\" = exec ] && [ \"$4\" = "
                           + shlex.quote(BINARY) + " ]; then exec sleep 60; fi\nexec /sbin/ip \"$@\"\n")
        wrapper.chmod(0o755)
        self.start(env=dict(os.environ, PATH=self.directory.name + ":" + os.environ["PATH"]))
        status, _ = self.put({"proxy_ports": [8080]})
        self.assertNotEqual(status, 200, "child without startup acknowledgment was accepted")
        self.assert_restored()

    def test_file_start_failure_restores_network(self):
        self.start(file_config={"proxy_ports": [8080], "rules": [
            {"target": "Request", "selector": {"method": "BAD METHOD"}, "actions": {}}]})
        self.assertNotEqual(self.process.wait(timeout=20), 0)
        self.assert_restored()

    def test_success_means_listener_bound_and_stop_preserves_nat(self):
        self.start()
        self.assertEqual(self.put(proxy_config("proxy-ready"))[0], 200)
        namespaces = run("ip", "netns", "list").splitlines()
        self.assertEqual(len(namespaces), 1)
        listeners = run("ip", "netns", "exec", namespaces[0].split()[0], "ss", "-ltnH")
        self.assertIn(":1025", listeners, "success returned before the listener bound")
        self.assert_connectivity("proxy-ready")
        self.assertEqual(self.put({})[0], 200)
        self.assert_restored()

    def test_safe_mode_stop_preserves_nat(self):
        self.start()
        self.assertEqual(self.put(proxy_config("proxy-safe", safe_mode=True))[0], 200)
        self.assert_connectivity("proxy-safe")
        self.assertEqual(self.put({})[0], 200)
        self.assert_restored()

    def test_active_reload_and_failed_reload_restore_network(self):
        self.start()
        self.assertEqual(self.put(proxy_config("proxy-a"))[0], 200)
        old_namespace = run("ip", "netns", "list").split()[0]
        old_links = self.links() - self.baseline_links
        self.assert_connectivity("proxy-a")
        self.assertEqual(self.put(proxy_config("proxy-b", safe_mode=True))[0], 200)
        namespaces = run("ip", "netns", "list").splitlines()
        self.assertEqual(len(namespaces), 1)
        self.assertNotEqual(namespaces[0].split()[0], old_namespace)
        self.assertTrue(old_links.isdisjoint(self.links()))
        self.assert_connectivity("proxy-b")
        status, _ = self.put({"proxy_ports": [18080], "rules": [
            {"target": "Request", "selector": {"method": "BAD METHOD"}, "actions": {}}]})
        self.assertEqual(status, 500)
        self.assert_restored()
        self.assertEqual(self.put(proxy_config("proxy-retry"))[0], 200)
        self.assert_connectivity("proxy-retry")
        self.assertEqual(self.put({})[0], 200)
        self.assert_restored()

    def test_stop_preserves_new_route(self):
        self.start()
        self.assertEqual(self.put({"proxy_ports": [8080]})[0], 200)
        run("ip", "route", "add", "blackhole", "198.51.100.0/24")
        self.assertEqual(self.put({})[0], 200)
        self.assertEqual(run("ip", "-4", "route", "show", "198.51.100.0/24"),
                         "blackhole 198.51.100.0/24")
        self.assert_restored()

    def test_stop_preserves_changed_route(self):
        run("ip", "route", "add", "blackhole", "203.0.113.0/24", "metric", "111")
        self.start()
        self.assertEqual(self.put({"proxy_ports": [8080]})[0], 200)
        run("ip", "route", "del", "blackhole", "203.0.113.0/24", "metric", "111")
        run("ip", "route", "add", "blackhole", "203.0.113.0/24", "metric", "222")
        self.assertEqual(self.put({})[0], 200)
        self.assertEqual(run("ip", "-4", "route", "show", "203.0.113.0/24"),
                         "blackhole 203.0.113.0/24 metric 222")
        self.assert_restored()

    def test_stop_preserves_new_default_route(self):
        original = json.loads(run("ip", "-j", "-4", "route", "show", "default"))[0]
        self.start()
        self.assertEqual(self.put({"proxy_ports": [8080]})[0], 200)
        run("ip", "route", "add", "default", "via", original["gateway"],
            "dev", original["dev"], "onlink", "metric", "222")
        self.assertEqual(self.put({})[0], 200)
        expected = {"gateway": original["gateway"], "dev": original["dev"], "metric": 222}
        self.assert_restored(expected_default=expected)

    def test_stop_preserves_deleted_route(self):
        run("ip", "route", "add", "blackhole", "203.0.113.0/24", "metric", "111")
        self.start()
        self.assertEqual(self.put({"proxy_ports": [8080]})[0], 200)
        run("ip", "route", "del", "blackhole", "203.0.113.0/24", "metric", "111")
        self.assertEqual(self.put({})[0], 200)
        self.assertEqual(run("ip", "-4", "route", "show", "203.0.113.0/24"), "")
        self.assert_restored()

    def test_stop_restores_connected_route_alternative(self):
        device, address, prefix = next(item for item in self.baseline_addresses if item[0] != "lo")
        subnet = str(ipaddress.ip_network(address + "/" + str(prefix), strict=False))
        run("ip", "route", "add", subnet, "dev", device, "metric", "111")
        original = run("ip", "-4", "route", "show", subnet)
        self.start()
        self.assertEqual(self.put({"proxy_ports": [8080]})[0], 200)
        self.assertEqual(self.put({})[0], 200)
        self.assertEqual(run("ip", "-4", "route", "show", subnet), original)
        self.assert_restored()

    def test_stop_restores_connected_route_without_kernel_default(self):
        device, address, prefix = next(item for item in self.baseline_addresses if item[0] != "lo")
        subnet = str(ipaddress.ip_network(address + "/" + str(prefix), strict=False))
        run("ip", "route", "add", subnet, "dev", device, "metric", "111")
        run("ip", "route", "del", subnet, "dev", device, "proto", "kernel")
        original = run("ip", "-4", "route", "show", subnet)
        self.start()
        self.assertEqual(self.put({"proxy_ports": [8080]})[0], 200)
        self.assertEqual(self.put({})[0], 200)
        self.assertEqual(run("ip", "-4", "route", "show", subnet), original)
        self.assert_restored()

    def test_stop_restores_multipath_route(self):
        self.assert_multipath_restored("198.51.100.0/24")

    def test_stop_restores_multipath_after_connected_route(self):
        self.assert_multipath_restored("10.0.0.0/24")

    def assert_multipath_restored(self, destination):
        original_default = json.loads(run("ip", "-j", "-4", "route", "show", "default"))[0]
        run("ip", "route", "add", destination,
            "nexthop", "via", original_default["gateway"], "dev", original_default["dev"], "weight", "1",
            "nexthop", "via", original_default["gateway"], "dev", original_default["dev"], "weight", "2")
        original = run("ip", "-4", "route", "show", destination)
        self.start()
        self.assertEqual(self.put({"proxy_ports": [8080]})[0], 200)
        self.assertEqual(self.put({})[0], 200)
        self.assertEqual(run("ip", "-4", "route", "show", destination), original)
        self.assert_restored()

    def assert_duplicate_nat_preserved(self, safe_mode):
        self.start()
        self.assertEqual(self.put(proxy_config("proxy-owned-nat", safe_mode=safe_mode))[0], 200)
        self.assert_connectivity("proxy-owned-nat")
        original_device = self.baseline_default.split(" dev ")[1].split()[0]
        commands = run("ebtables", "-t", "nat", "-L", "--Lx").splitlines()
        owned = next(shlex.split(line) for line in commands if "-j dnat" in line)
        suffix = owned[owned.index("-j"):]
        foreign = ["ebtables", "-t", "nat", "-I", "PREROUTING", "1",
                   "-i", original_device] + suffix + ["-c", "17", "1700"]
        run(*foreign)
        foreign_spec = run("ebtables", "-t", "nat", "-L", "PREROUTING", "--Lx").splitlines()[0]
        self.assertEqual(self.put({})[0], 200)
        current = run("ebtables", "-t", "nat", "-L", "PREROUTING", "--Lx", "--Lc").splitlines()
        rule, counters = current[0].split("-c ")
        self.assertEqual(rule.strip(), foreign_spec.strip())
        packets, size = map(int, counters.split())
        self.assertGreaterEqual(packets, 17)
        self.assertGreaterEqual(size, 1700)
        run("ebtables", "-t", "nat", "-D", "PREROUTING", "-i", original_device, *suffix)
        self.assert_restored()

    def nat_failure_environment(self, operation, chain, once=False):
        wrapper = Path(self.directory.name) / "ebtables"
        marker = Path(self.directory.name) / "nat-failure"
        wrapper.write_text("#!/usr/bin/env python3\nimport os,sys\nfrom pathlib import Path\n"
                           "args=sys.argv[1:]\n"
                           "operation=" + repr(operation) + "\nchain=" + repr(chain) + "\n"
                           "marker=Path(" + repr(str(marker)) + ")\n"
                           "if operation in args:\n"
                           "    target=args[args.index(operation)+1]\n"
                           "    match=(target == 'PREROUTING') if chain == 'jump' else target not in ('PREROUTING','OUTPUT','POSTROUTING')\n"
                           "    if match and (" + repr(not once) + " or not marker.exists()):\n"
                           "        marker.touch()\n        sys.exit(42)\n"
                           "os.execv('/usr/sbin/ebtables-legacy',['ebtables-legacy']+args)\n")
        wrapper.chmod(0o755)
        return dict(os.environ, PATH=self.directory.name + ":" + os.environ["PATH"])

    def test_review_nat_chain_rule_failure_rolls_back(self):
        self.start(env=self.nat_failure_environment("-A", "chain"))
        self.assertEqual(self.put(proxy_config("proxy-nat-failure"))[0], 500)
        self.assert_restored()

    def test_review_nat_jump_failure_rolls_back(self):
        self.start(env=self.nat_failure_environment("-A", "jump"))
        self.assertEqual(self.put(proxy_config("proxy-nat-failure"))[0], 500)
        self.assert_restored()

    def test_review_nat_jump_cleanup_can_retry(self):
        observer = self.gateway_ping_observer()
        self.start(env=self.nat_failure_environment("-D", "jump", once=True))
        self.assertEqual(self.put(proxy_config("proxy-nat-retry"))[0], 200)
        self.assert_connectivity("proxy-nat-retry")
        self.assert_one_gateway_ping(observer)
        self.assertEqual(self.put({})[0], 500)
        self.assertIn("-j dnat", run("ebtables", "-t", "nat", "-L", "--Lx"))
        self.assertEqual(self.gateway_pings(observer), [])
        self.assertEqual(self.put({})[0], 200)
        self.assert_restored()

    def test_review_nat_chain_cleanup_can_retry(self):
        self.start(env=self.nat_failure_environment("-X", "chain", once=True))
        self.assertEqual(self.put(proxy_config("proxy-nat-retry"))[0], 200)
        self.assert_connectivity("proxy-nat-retry")
        self.assertEqual(self.put({})[0], 500)
        self.assertNotIn("-j dnat", run("ebtables", "-t", "nat", "-L", "--Lx"))
        self.assertEqual(self.put({})[0], 200)
        self.assert_restored()

    def test_review_duplicate_nat_normal_stop(self):
        self.assert_duplicate_nat_preserved(False)

    def test_review_duplicate_nat_safe_stop(self):
        self.assert_duplicate_nat_preserved(True)

    def add_backup_default(self):
        run("ip", "link", "add", "backup0", "type", "dummy")
        run("ip", "address", "add", "192.0.2.10/24", "dev", "backup0")
        run("ip", "link", "set", "backup0", "up")
        run("ip", "route", "add", "default", "via", "192.0.2.1", "dev", "backup0", "metric", "1000")
        self.baseline_addresses = self.addresses()
        self.baseline_links = self.links()
        self.baseline_default = run("ip", "-4", "route", "show", "default")
        return run("ip", "-4", "route", "show")

    def test_review_preserves_preexisting_backup_default(self):
        routes = self.add_backup_default()
        self.start()
        self.assertEqual(self.put(proxy_config("proxy-backup-route"))[0], 200)
        self.assert_connectivity("proxy-backup-route")
        self.assertEqual(self.put({})[0], 200)
        self.assertEqual(run("ip", "-4", "route", "show"), routes)
        self.assert_restored()

    def test_review_failed_start_preserves_backup_default(self):
        routes = self.add_backup_default()
        self.start()
        self.assertEqual(self.put({"proxy_ports": [18080], "rules": [
            {"target": "Request", "selector": {"method": "BAD METHOD"}, "actions": {}}]})[0], 500)
        self.assertEqual(run("ip", "-4", "route", "show"), routes)
        self.assert_restored()

    def test_review_new_default_wins_with_backup(self):
        original = json.loads(run("ip", "-j", "-4", "route", "show", "default"))[0]
        self.add_backup_default()
        self.start()
        self.assertEqual(self.put(proxy_config("proxy-backup-new-route"))[0], 200)
        self.assert_connectivity("proxy-backup-new-route")
        run("ip", "route", "add", "default", "via", original["gateway"],
            "dev", original["dev"], "onlink", "metric", "222")
        self.baseline_default = "\n".join(
            line for line in run("ip", "-4", "route", "show", "default").splitlines()
            if " dev " + original["dev"] + " " in line or " dev backup0 " in line)
        self.assertEqual(self.put({})[0], 200)
        defaults = json.loads(run("ip", "-j", "-4", "route", "show", "default"))
        self.assertEqual([(route["dev"], route.get("metric", 0)) for route in defaults],
                         [(original["dev"], 222), ("backup0", 1000)])
        self.assert_restored()

    def test_review_existing_nat_chain_is_not_owned(self):
        wrapper = Path(self.directory.name) / "ebtables"
        marker = Path(self.directory.name) / "foreign-chain"
        wrapper.write_text("#!/usr/bin/env python3\nimport os,sys,subprocess\nfrom pathlib import Path\n"
                           "args=sys.argv[1:]\n"
                           "if '-N' in args:\n"
                           "    chain=args[args.index('-N')+1]\n"
                           "    subprocess.check_call(['/usr/sbin/ebtables-legacy']+args)\n"
                           "    subprocess.check_call(['/usr/sbin/ebtables-legacy','-t','nat','-A',chain,'-j','ACCEPT','-c','17','1700'])\n"
                           "    Path(" + repr(str(marker)) + ").write_text(chain)\n"
                           "os.execv('/usr/sbin/ebtables-legacy',['ebtables-legacy']+args)\n")
        wrapper.chmod(0o755)
        self.start(env=dict(os.environ, PATH=self.directory.name + ":" + os.environ["PATH"]))
        self.assertEqual(self.put(proxy_config("proxy-chain-collision"))[0], 500)
        chain = marker.read_text()
        foreign = run("ebtables", "-t", "nat", "-L", chain, "--Lx", "--Lc")
        self.assertIn("-j ACCEPT -c 17 1700", foreign)
        run("ebtables", "-t", "nat", "-F", chain)
        run("ebtables", "-t", "nat", "-X", chain)
        self.assert_restored()

    def test_patch_preserves_duplicate_headers(self):
        self.start()
        config = {"proxy_ports": [18080], "rules": [
            {"target": "Request", "selector": {}, "actions": {"patch": {
                "headers": [["X-Repeated", "first"], ["x-repeated", "second"]]}}},
            {"target": "Response", "selector": {}, "actions": {"patch": {
                "headers": [["Set-Cookie", "one=1"], ["set-cookie", "two=2"]]}}},
        ]}
        self.assertEqual(self.put(config)[0], 200)
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        with opener.open(os.environ["TPROXY_TEST_URL"] + "echo", timeout=5) as response:
            self.assertEqual(json.loads(response.read())["headers"], ["first", "second"])
            self.assertEqual(response.headers.get_all("Set-Cookie"), ["one=1", "two=2"])
        self.assertEqual(self.put({})[0], 200)
        self.assert_restored()

    def test_query_replacement_preserves_other_duplicates(self):
        self.start()
        config = {"proxy_ports": [18080], "rules": [
            {"target": "Request", "selector": {}, "actions": {"replace": {
                "queries": {"replace": "new value"}}}},
        ]}
        self.assertEqual(self.put(config)[0], 200)
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        with opener.open(os.environ["TPROXY_TEST_URL"] +
                         "echo?tag=a&replace=old&tag=b&empty=&replace=older", timeout=5) as response:
            path = json.loads(response.read())["path"]
            self.assertEqual(parse_qsl(urlsplit(path).query, keep_blank_values=True),
                             [("tag", "a"), ("tag", "b"), ("empty", ""), ("replace", "new value")])
        self.assertEqual(self.put({})[0], 200)
        self.assert_restored()

    def test_stdin_eof_before_configuration(self):
        self.start()
        self.process.stdin.close()
        self.assertEqual(self.process.wait(timeout=10), 0)
        self.assert_restored()

    def test_stdin_eof_restores_network(self):
        self.start()
        self.assertEqual(self.put(proxy_config("proxy-eof"))[0], 200)
        self.assert_connectivity("proxy-eof")
        self.process.stdin.close()
        self.assertEqual(self.process.wait(timeout=10), 0)
        self.assert_restored()

    def test_stdin_eof_safe_mode_restores_network(self):
        self.start()
        self.assertEqual(self.put(proxy_config("proxy-eof-safe", safe_mode=True))[0], 200)
        self.assert_connectivity("proxy-eof-safe")
        self.process.stdin.close()
        self.assertEqual(self.process.wait(timeout=10), 0)
        self.assert_restored()

    def test_stdin_eof_during_request_restores_network(self):
        self.start()
        self.assertEqual(self.put(proxy_config("proxy-eof-partial"))[0], 200)
        self.assert_connectivity("proxy-eof-partial")
        self.process.stdin.write(b"PUT / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 100\r\n\r\n{")
        self.process.stdin.close()
        self.assertEqual(self.process.wait(timeout=10), 0)
        self.assert_restored()

    def test_stdin_eof_during_header_restores_network(self):
        self.start()
        self.assertEqual(self.put(proxy_config("proxy-eof-header"))[0], 200)
        self.assert_connectivity("proxy-eof-header")
        self.process.stdin.write(b"PUT / HTTP/1.1\r\nHost:")
        self.process.stdin.close()
        self.assertEqual(self.process.wait(timeout=10), 0)
        self.assert_restored()

    def test_stdin_eof_after_failed_reload(self):
        self.start()
        self.assertEqual(self.put(proxy_config("proxy-eof-failure"))[0], 200)
        self.assert_connectivity("proxy-eof-failure")
        self.assertEqual(self.put({"proxy_ports": [18080], "rules": [
            {"target": "Request", "selector": {"method": "BAD METHOD"}, "actions": {}}]})[0], 500)
        self.process.stdin.close()
        self.assertEqual(self.process.wait(timeout=10), 0)
        self.assert_restored()

    def test_connection_close_accepts_reload_and_stop(self):
        self.start()
        self.assertEqual(self.put(proxy_config("proxy-close-first"), connection_close=True)[0], 200)
        self.assert_connectivity("proxy-close-first")
        time.sleep(0.2)
        self.assertEqual(self.put(proxy_config("proxy-close-reloaded"), connection_close=True)[0], 200)
        self.assert_connectivity("proxy-close-reloaded")
        time.sleep(0.2)
        self.assertEqual(self.put({})[0], 200)
        self.assertIsNone(self.process.poll())
        self.assert_restored()

    def test_connection_close_batched_reload_and_stop(self):
        self.start()
        self.process.stdin.write(
            self.request(proxy_config("proxy-close-batch-first"), connection_close=True)
            + self.request(proxy_config("proxy-close-batch-reloaded"), connection_close=True)
            + self.request({}))
        self.process.stdin.flush()
        self.assertEqual([self.read_response()[0] for _ in range(3)], [200, 200, 200])
        self.assertIsNone(self.process.poll())
        self.assert_restored()

    def test_connection_close_repeated_in_batch(self):
        self.start()
        requests = [self.request({}, connection_close=True) for _ in range(8)]
        requests.append(self.request(proxy_config("proxy-close-repeated")))
        self.process.stdin.write(b"".join(requests))
        self.process.stdin.flush()
        self.assertEqual([self.read_response()[0] for _ in requests], [200] * 9)
        self.assert_connectivity("proxy-close-repeated")
        self.assertEqual(self.put({})[0], 200)
        self.assert_restored()

    def test_connection_close_batch_crosses_read_buffer(self):
        self.start()
        requests = [self.request({}, connection_close=True),
                    self.request(proxy_config("proxy-close-large"), connection_close=True,
                                 padding=256 * 1024)]
        requests.extend(self.request({}, connection_close=True, padding=4096) for _ in range(8))
        requests.append(self.request(proxy_config("proxy-close-after-large")))
        self.process.stdin.write(b"".join(requests))
        self.process.stdin.flush()
        self.assertEqual([self.read_response()[0] for _ in requests], [200] * 11)
        self.assert_connectivity("proxy-close-after-large")
        self.assertEqual(self.put({})[0], 200)
        self.assert_restored()

    def test_connection_close_batch_before_eof(self):
        self.start()
        self.process.stdin.write(
            self.request(proxy_config("proxy-close-eof-first"), connection_close=True)
            + self.request(proxy_config("proxy-close-eof-last"), connection_close=True)
            + self.request({}, connection_close=True))
        self.process.stdin.close()
        self.assertEqual([self.read_response()[0] for _ in range(3)], [200, 200, 200])
        self.assertEqual(self.process.wait(timeout=10), 0)
        self.assert_restored()

    def test_connection_close_preserves_error_responses(self):
        self.start()
        invalid = {"proxy_ports": [18080], "rules": [
            {"target": "Request", "selector": {"method": "BAD METHOD"}, "actions": {}}]}
        self.process.stdin.write(
            self.request("not a configuration", connection_close=True)
            + self.request(invalid, connection_close=True)
            + self.request(proxy_config("proxy-close-errors")))
        self.process.stdin.flush()
        responses = [self.read_response() for _ in range(3)]
        self.assertEqual([status for status, _ in responses], [400, 500, 200])
        self.assertIn("invalid type", responses[0][1])
        self.assertIn("invalid HTTP method", responses[1][1])
        self.assert_connectivity("proxy-close-errors")
        self.assertEqual(self.put({})[0], 200)
        self.assert_restored()

    def test_connection_close_waits_for_stdin_eof(self):
        self.start()
        self.assertEqual(self.put(proxy_config("proxy-close"), connection_close=True)[0], 200)
        self.assert_connectivity("proxy-close")
        self.assertIsNone(self.process.poll())
        self.process.stdin.close()
        self.assertEqual(self.process.wait(timeout=10), 0)
        self.assert_restored()

    def test_sigterm_restores_network(self):
        self.start()
        self.assertEqual(self.put({"proxy_ports": [8080]})[0], 200)
        self.process.send_signal(signal.SIGTERM)
        self.assertEqual(self.process.wait(timeout=10), 0)
        self.assert_restored()

    def test_sigint_restores_network(self):
        self.start()
        self.assertEqual(self.put({"proxy_ports": [8080]})[0], 200)
        self.process.send_signal(signal.SIGINT)
        self.assertEqual(self.process.wait(timeout=10), 0)
        self.assert_restored()


if __name__ == "__main__":
    unittest.main(verbosity=2)
