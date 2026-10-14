#!/usr/bin/env bash
set -euo pipefail
image="${1:-chaos-tproxy-lifecycle-test}"
results="${2:-$(mktemp -d)}"
mkdir -p "$results"
: > "$results/results.tsv"
run_id="chaos-tproxy-lifecycle-$$"
network="$run_id"
backend="${run_id}-backend"
current=""
cases=$(docker run --rm --entrypoint python3 "$image" -c '
import ast
module = ast.parse(open("/tests/test_lifecycle.py").read())
for node in module.body:
    if isinstance(node, ast.ClassDef) and node.name == "LifecycleTests":
        for member in node.body:
            if isinstance(member, ast.FunctionDef) and member.name.startswith("test_"):
                print(member.name)
')
if [ -z "$cases" ]; then
    printf 'No lifecycle tests found in %s\n' "$image" >&2
    exit 1
fi
cleanup() {
    if [ -n "$current" ]; then docker rm -f "$current" >/dev/null 2>&1 || true; fi
    docker rm -f "$backend" >/dev/null 2>&1 || true
    docker network rm "$network" >/dev/null 2>&1 || true
}
trap cleanup EXIT HUP INT TERM
docker network create "$network" >/dev/null
docker run -d --rm --network "$network" --name "$backend" "$image" /tests/lifecycle_backend.py >/dev/null
backend_ip=$(docker inspect --format '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$backend")
docker exec "$backend" python3 -c '
import time, urllib.request
for attempt in range(100):
    try:
        assert urllib.request.urlopen("http://127.0.0.1:18080/", timeout=1).read() == b"network-ok\n"
        break
    except OSError:
        time.sleep(0.05)
else:
    raise RuntimeError("HTTP fixture did not become ready")
'
failed=0
for case in $cases; do
    current="${run_id}-${case}"
    result=0
    docker run --rm --privileged --network "$network" --name "$current" \
        -e CHAOS_TPROXY_TEST_ISOLATED=1 -e "TPROXY_TEST_URL=http://${backend_ip}:18080/" \
        "$image" /tests/test_lifecycle.py "LifecycleTests.$case" > "$results/$case.log" 2>&1 || result=$?
    current=""
    printf '%s\t%s\n' "$case" "$result" >> "$results/results.tsv"
    if [ "$result" -eq 0 ]; then
        printf '%s PASS\n' "$case"
    else
        printf '%s FAIL\n' "$case"
        tail -n 20 "$results/$case.log"
        failed=1
    fi
done
printf 'Results saved in %s\n' "$results"
exit "$failed"
