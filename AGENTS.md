# Working on chaos-tproxy

## Commits

Every commit must include a `Signed-off-by` trailer that matches the author's name and email. Use `git commit --signoff` for new commits. To add a missing sign-off to the latest commit, use `git commit --amend --no-edit --signoff`.

Preserve existing sign-offs when rebasing. Before pushing a PR, verify that every commit in the PR has the required sign-off so the DCO check can pass.

## Project map

- `chaos-tproxy-controller/` parses user configuration, manages Linux networking, starts the proxy subprocess, and serves interactive configuration over standard input and output.
- `chaos-tproxy-proxy/` handles transparent TCP connections, HTTP selectors and actions, TLS, and the Unix socket startup exchange.
- `tests/` contains Rust integration tests and a Python HTTP fixture.
- `podnetmock/` is a separate Go package for UDP probes and packet capture. Cargo does not test it.
- `config-examples/` contains user configuration examples.
- The root Cargo package builds `chaos-tproxy` from the controller's `src/main.rs`. The controller package also builds `chaos-tproxy-controller-bin` from that file. Check both entry points when changing the CLI.

## Build and checks

Run Rust checks on Linux. The controller uses Linux netlink and network namespaces. The transparent socket implementation uses IPv4 and `IP_TRANSPARENT`.

`rust-toolchain` selects nightly. The Dockerfile pins Rust 1.94.1. `rustfmt.toml` uses nightly formatting options. Keep these differences in mind when comparing local, Docker, and CI results.

Run checks appropriate to the change from the repository root.

```sh
cargo build --locked --workspace
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo +nightly fmt --all -- --check
```

For HTTP actions, selectors, or TLS parsing, run the proxy library tests.

```sh
cargo test --locked -p chaos-tproxy-proxy --lib
```

For controller configuration or network command generation, run the tests that do not change networking.

```sh
cargo test --locked -p chaos-tproxy-controller --lib proxy::config::tests
cargo test --locked -p chaos-tproxy-controller --lib proxy::net::bridge::tests
```

`make image` runs all nonignored proxy and controller library tests and builds release binaries in Docker. `make release` copies the binary to `./tproxy`. A native Cargo build produces `target/debug/chaos-tproxy`. The current `make run` target expects `target/debug/tproxy`, so invoke the Cargo binary directly.

For Go changes, install the libpcap headers and run `go test ./...` on Linux with packet capture permissions. `TestMonitor` uses the Linux `lo` interface. The Rust CI workflow does not run the Go tests.

The ignored HTTP content-length test needs `tests/integrations/checker.py` listening on port 8080. Start that fixture separately, then run the test from `tests/`.

```sh
cargo test --locked --release --test integration -- --ignored test_http_action::test_http_content_length_replace
```

Stop the fixture afterward. `tests/exec.py` starts it but does not stop it or propagate the Cargo exit status. Do not use that script's exit status as proof of success. The ignored route test deletes routes. Run it only in a disposable Linux environment.

## Change constraints

- Keep user-facing `RawConfig` in the controller distinct from the proxy's internal `RawConfig`. The controller converts a port array into a comma-separated string for the subprocess.
- File input accepts `.json` and `.yaml`. Interactive input accepts JSON in HTTP `PUT` requests over standard input, with responses on standard output. Logs belong on standard error.
- Preserve configuration behavior deliberately. Omitting `proxy_ports` or setting it to `null` stops interception. An empty array currently attempts network setup and is not a stop command.
- `interface`, `listen_port`, `proxy_mark`, `ignore_mark`, and `route_table` are accepted compatibility fields but ignored. Do not document them as effective settings.
- Keep selectors, rule ordering, HTTP framing, raw TCP fallback, and TLS behavior consistent across configuration conversion and runtime handling. Update examples and tests when changing these contracts.
- Body replacement and JSON merge remove `Content-Length`. There is no supported `update_content_length` option. TLS key parsing currently accepts PEM RSA PKCS#1 keys.
- Cargo dependencies include Git forks of Hyper and hyper-rustls. The proxy uses fork-specific APIs. Do not replace those dependencies with upstream crates without checking callers and behavior.
- Preserve `Cargo.lock` unless a dependency change requires an update.

## Network verification

Run the controller only in a disposable Linux VM or container with an IPv4 default interface and the required privileges. It changes addresses, bridges, routes, firewall rules, and resolver files in its network namespace. Cleanup removes only the run's outer NAT rule and restores routes displaced by setup without replacing current routes. Do not test interception in the host network namespace.

`tests/integrations/run_lifecycle.sh` runs the lifecycle suite in separate disposable privileged containers with a dedicated network and HTTP backend. Build its runtime image using `tests/integrations/Dockerfile.lifecycle` after `make image`. See README for the commands. The Python suite requires Linux root and `CHAOS_TPROXY_TEST_ISOLATED=1` to prevent accidental direct execution.

For lifecycle changes, verify apply, reload, failed startup, stop, and restoration on that Linux environment. For HTTP changes, send real requests through the proxy. Library tests and Docker builds do not verify network interception or restoration. Report the checks actually run and any environment limits.
