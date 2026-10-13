#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."
python3 http/checker.py &
fixture_pid=$!
trap 'kill "$fixture_pid" 2>/dev/null || true; wait "$fixture_pid" 2>/dev/null || true' EXIT

python3 - "$fixture_pid" <<'PYTHON'
import os
import socket
import sys
import time

fixture_pid = int(sys.argv[1])
deadline = time.monotonic() + 10
while time.monotonic() < deadline:
    os.kill(fixture_pid, 0)
    try:
        with socket.create_connection(("127.0.0.1", 8080), timeout=0.2):
            break
    except OSError:
        time.sleep(0.1)
else:
    sys.exit("HTTP fixture did not become ready within 10 seconds")
PYTHON
kill -0 "$fixture_pid"

cargo test --locked --workspace --release --test integration -- --ignored --exact test_http_action::test_http_content_length_replace
