# AI agent instructions for chaos-tproxy

Use these instructions when inspecting, modifying, or validating this repository. Keep user-facing installation and configuration guidance in `README.md`.

## Commits

Every commit must include a `Signed-off-by` trailer that matches the author's name and email. Use `git commit --signoff` for new commits. To add a missing sign-off to the latest commit, use `git commit --amend --no-edit --signoff`.

Preserve existing sign-offs when rebasing. Before pushing a PR, verify that every commit in the PR has the required sign-off so the DCO check can pass.

## Repository map

- `chaos-tproxy-controller/` parses user configuration, manages Linux networking, starts the proxy subprocess, and handles configuration over standard input and output.
- `chaos-tproxy-proxy/` implements transparent TCP connections, HTTP selectors and actions, TLS, and the Unix socket startup exchange.
- `tests/` contains Rust integration tests and the Python HTTP fixture.
- `test-support/` contains shared CLI and stdio process tests for both Rust binaries.
- `podnetmock/` contains Go UDP probes and packet capture code. It is outside the Cargo workspace.
- `config-examples/` contains user configuration examples.

The root package builds `chaos-tproxy` from `chaos-tproxy-controller/src/main.rs`. The controller package builds `chaos-tproxy-controller-bin` from the same file. Check both binaries when changing the CLI.

## Select and run checks

Run Rust checks on Linux from the repository root. The controller depends on Linux netlink and network namespaces. Transparent sockets use IPv4 and `IP_TRANSPARENT`.

The repository selects nightly Rust. The Dockerfile pins Rust 1.94.1. Formatting requires nightly because `rustfmt.toml` enables unstable options.

Run the following checks as appropriate for the change.

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

For controller configuration or network command generation, run the following tests. These tests do not modify networking.

```sh
cargo test --locked -p chaos-tproxy-controller --lib proxy::config::tests
cargo test --locked -p chaos-tproxy-controller --lib proxy::net::bridge::tests
```

`make image` runs all nonignored proxy and controller library tests and builds the release binaries in Docker. `make release` copies the main binary to `./tproxy`. Native debug builds produce `target/debug/chaos-tproxy`; `make run` invokes that binary.

### HTTP integration test

Run the following command from the repository root. The script starts the Python fixture on port 8080, runs the ignored content-length test, and stops the fixture on exit.

```sh
bash tests/http/run_http_test.sh
```

The ignored route test deletes routes. Run it only in a disposable Linux environment.

### Lifecycle integration test

Run the following commands from the repository root with Docker available.

Load the required networking modules on the disposable Linux Docker host before running the suite. Test containers share its kernel and do not contain its module files. The lifecycle CI job loads these modules explicitly before building the images.

```sh
for module in ebtable_nat ebtable_broute ebt_ip ebt_dnat ebt_redirect \
  iptable_mangle xt_socket xt_TPROXY xt_multiport xt_tcpudp xt_mark; do
  sudo modprobe "$module"
done
```

```sh
make image
docker build -f tests/integrations/Dockerfile.lifecycle \
  --build-arg TPROXY_IMAGE=chaos-mesh/tproxy -t chaos-tproxy-lifecycle-test .
bash tests/integrations/run_lifecycle.sh chaos-tproxy-lifecycle-test
```

The suite creates a dedicated Docker network and HTTP backend, then uses a separate privileged Linux container for each case. It checks startup failures, apply, reload, normal and safe-mode stop, SIGINT and SIGTERM, preservation of unrelated NAT rules and route changes, and real HTTP interception and restoration. It prints the results directory and removes its containers and network on exit. It does not use the host network namespace. The Python suite requires Linux root and `CHAOS_TPROXY_TEST_ISOLATED=1` to prevent accidental direct execution.

The CI test workflow runs the suite as the independent `Lifecycle integration tests` job for pull requests, pushes to `master`, and manual runs. It saves per-case logs and `results.tsv` as the `lifecycle-results` artifact for seven days, including when a test fails.

### Go tests

For changes to `podnetmock/`, install libpcap headers and run `go test ./...` from that directory on Linux with packet capture permissions. `TestMonitor` uses the `lo` interface. The Rust CI workflow does not run Go tests.

## Change constraints

- Keep the controller's user-facing `RawConfig` separate from the proxy's internal `RawConfig`. The controller converts the port array to a comma-separated string for the subprocess.
- Preserve `.json` and `.yaml` file input and JSON configuration through HTTP `PUT` requests over standard input. Write interactive responses to standard output and logs to standard error.
- Preserve stop semantics. An omitted or `null` `proxy_ports` value stops interception. An empty array attempts network setup and does not stop interception.
- Treat `interface`, `listen_port`, `proxy_mark`, `ignore_mark`, and `route_table` as ignored compatibility fields. Do not describe them as effective settings.
- Keep selector behavior, rule order, HTTP framing, raw TCP fallback, and TLS behavior consistent between configuration conversion and runtime handling. Update examples and tests when changing these contracts.
- Preserve removal of `Content-Length` after body replacement or JSON merge. The `update_content_length` option is not supported. TLS key parsing accepts PEM RSA PKCS#1 keys.
- Retain the Git forks of Hyper and hyper-rustls unless replacing them is part of the change. Verify fork-specific callers and behavior before using upstream crates.
- Update `Cargo.lock` only when a dependency change requires it.

## Verify network behavior

Run the controller only in a disposable Linux VM or container with an IPv4 default interface and the required privileges. It modifies interface addresses, bridges, routes, firewall rules, and resolver files. Cleanup removes only the run's NAT jump and chain and restores routes displaced by setup. Preserve unchanged baseline alternatives, and let routes added or changed during the run take precedence. Do not test interception in the host network namespace.

For lifecycle changes, verify configuration apply, reload, failed startup, stop, and network restoration. For HTTP changes, send real requests through the proxy. Library tests and Docker builds do not verify interception or restoration.

When reporting completed work, state which checks ran, their results, and which checks remain unverified because of environment limits.
