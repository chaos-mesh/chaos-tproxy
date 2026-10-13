import json
import subprocess
import sys
import time
import urllib.error
import urllib.request

name = sys.argv[1]
opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

def port(container_port):
    mapping = subprocess.check_output(['docker', 'port', name, str(container_port)], text=True).strip()
    return mapping.rsplit(':', 1)[1]

control = 'http://127.0.0.1:' + port(18080)
origin = 'http://127.0.0.1:' + port(8080)

def request(path, config=None):
    data = None if config is None else json.dumps(config).encode()
    req = urllib.request.Request(control + path, data=data, method='GET' if data is None else 'PUT')
    try:
        with opener.open(req, timeout=30) as response:
            return response.read()
    except urllib.error.HTTPError as err:
        raise AssertionError(f"config request failed: HTTP {err.code}, body: {err.read().decode()}") from err

def state():
    return json.loads(request('/state'))

def ipv4_addresses(snapshot):
    interface = next(entry for entry in snapshot['address'] if entry['ifname'] == 'eth0')
    return sorted((entry['local'], entry['prefixlen']) for entry in interface['addr_info'] if entry['family'] == 'inet')

def routes(snapshot):
    return sorted(json.dumps(route, sort_keys=True) for route in snapshot['route'])

def body():
    with opener.open(origin, timeout=5) as response:
        return response.read().decode()

def eventually(expected):
    deadline = time.monotonic() + 15
    last = None
    while time.monotonic() < deadline:
        try:
            last = body()
            if last == expected:
                return
        except (urllib.error.URLError, TimeoutError) as err:
            last = str(err)
        time.sleep(0.1)
    raise AssertionError(f'expected body {expected!r}, got {last!r}')

def config(value, role=None):
    result = {'proxy_ports': [8080], 'rules': [{
        'target': 'Response', 'selector': {'port': 8080},
        'actions': {'replace': {'body': {'contents': {'type': 'TEXT', 'value': value}}}}
    }]}
    if role is not None:
        result['role'] = role
    return result

deadline = time.monotonic() + 10
while True:
    try:
        before = state()
        break
    except urllib.error.URLError:
        if time.monotonic() >= deadline:
            raise
        time.sleep(0.1)
interface = next(entry for entry in before['address'] if entry['ifname'] == 'eth0')
ipv4 = [entry for entry in interface['addr_info'] if entry['family'] == 'inet']
assert ipv4, interface
assert any(entry['family'] == 'inet6' for entry in interface['addr_info']), interface
eventually('original')
print('original traffic passed', flush=True)
request('/config', config('injected'))
eventually('injected')
active = state()
eth0 = next(entry for entry in active['address'] if entry['ifname'] == 'eth0')
assert not any(entry['family'] == 'inet' for entry in eth0['addr_info']), eth0
print('injection passed; original interface has no IPv4', flush=True)
request('/config', {'rules': []})
eventually('original')
restored = state()
assert ipv4_addresses(restored) == ipv4_addresses(before), (before, restored)
assert routes(restored) == routes(before), (before['route'], restored['route'])
print('pause passed; original IPv4 and routes restored', flush=True)
request('/config', config('resumed'))
eventually('resumed')
print('resume passed', flush=True)
request('/config', config('updated'))
eventually('updated')
print('active rule reload passed', flush=True)
request('/config', {'rules': [], 'role': 'Client'})
eventually('original')
print('stop with role passed', flush=True)
request('/config', config('role-injected', 'Server'))
eventually('role-injected')
request('/config', config('role-updated', 'Server'))
eventually('role-updated')
print('Server role reload passed', flush=True)
request('/config', {'rules': []})
eventually('original')
after = state()
assert ipv4_addresses(after) == ipv4_addresses(before), (before, after)
assert routes(after) == routes(before), (before['route'], after['route'])
print('final stop passed; original IPv4 and routes restored', flush=True)
