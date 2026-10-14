import ipaddress
import json
import os
from pathlib import Path
import select
import shlex
import signal
import subprocess
import sys
import tempfile
import time
import unittest
import urllib.request


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
        self.process = subprocess.Popen(args, stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE, stderr=self.logs,
                                        start_new_session=True, env=env)
        return self.process

    def put(self, config):
        body = json.dumps(config, separators=(",", ":")).encode()
        request = (b"PUT / HTTP/1.1\r\nHost: localhost\r\nContent-Length: "
                   + str(len(body)).encode() + b"\r\n\r\n" + body)
        self.process.stdin.write(request)
        self.process.stdin.flush()
        response = b""
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
        return int(header.split()[1]), body.decode(errors="replace")

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
