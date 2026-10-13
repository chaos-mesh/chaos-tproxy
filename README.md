# chaos-tproxy

chaos-tproxy injects faults into HTTP requests and responses without changing application proxy settings. It uses Linux TPROXY to intercept IPv4 TCP traffic and can abort or delay HTTP exchanges, replace fields, and merge JSON bodies.

## Prepare a Linux environment

Run chaos-tproxy as root in a disposable Linux VM or container with an IPv4 default interface. The controller changes bridges, interface addresses, routes, resolver files, and firewall rules in its network namespace. Cleanup currently flushes that namespace's entire ebtables NAT table. Avoid running it in your host network namespace.

The runtime needs these commands on `PATH`.

```sh
command -v ip sysctl arp iptables ebtables ebtables-legacy
```

Install them through your distribution's packages. `ip` comes from iproute2, and `arp` comes from net-tools. Both `ebtables` and `ebtables-legacy` are invoked by the controller. The BROUTING rule specifically requires the legacy implementation. The original setup also uses legacy iptables.

The kernel needs bridge, network namespace, policy routing, and TPROXY support. Relevant modules include `ebtables`, `ebtable_broute`, `iptable_mangle`, `xt_TPROXY`, and `xt_socket`. Check with `lsmod` or your kernel configuration if the features are built in.

Ensure that DNS remains reachable after the controller changes networking. A resolver bound only to loopback, such as a local dnsmasq instance, can become unreachable. Use a resolver accessible from the test environment.

## Install or build

Download the archive for your Linux architecture from [GitHub releases](https://github.com/chaos-mesh/chaos-tproxy/releases). Release archives are named `tproxy-x86_64.tar.gz` and `tproxy-aarch64.tar.gz`. Each contains a binary named `tproxy`.

To build from source, use the repository's Rust toolchain and run this command from the repository root.

```sh
cargo build --locked --release --workspace
```

The main binary is `target/release/chaos-tproxy`. `rust-toolchain` selects nightly. The Docker build uses Rust 1.94.1 instead.

To build the image and copy its release binary to `./tproxy`, run this command.

```sh
make release
```

`make release` builds the image, runs the focused Rust tests in the Dockerfile, and copies the binary. It does not publish a GitHub release. The build image is not a complete runtime environment for network interception.

The commands below use the release archive's `./tproxy`. Substitute `./target/release/chaos-tproxy` for a native source build.

## Delay matching requests

Create a configuration that delays GET requests to `/example*` on destination port 8080 by five seconds.

```sh
cat > example.yaml <<'YAML'
proxy_ports: [8080]
rules:
  - target: Request
    selector:
      path: /example*
      method: GET
    actions:
      delay: 5s
YAML
sudo ./tproxy -v example.yaml
```

Send a matching request through the intercepted interface to an HTTP server listening on port 8080. Loopback traffic is not a substitute for this network path. Press Ctrl-C or send SIGTERM to stop the controller and trigger network cleanup.

## Configure faults

File mode accepts `.json` and `.yaml` files. The `.yml` extension is not accepted. See [configuration examples](config-examples/) for JSON, YAML, and TLS configurations.

The controller accepts these settings.

| Field | Behavior |
| --- | --- |
| `proxy_ports` | Destination TCP ports to intercept. Omit the field or set it to `null` to stop interception. An empty array currently attempts setup and is not a stop command. |
| `rules` | HTTP fault rules. Omitted rules default to an empty list. |
| `role` | `Client` applies rules when the connection source matches an IPv4 address on the default interface. `Server` matches the destination against those addresses. Omit it to apply rules in both directions. |
| `safe_mode` | Defaults to `false`. When enabled, bypasses TCP traffic with source or destination ports 1–1025 for the default interface's IPv4 subnet. This can bypass ports selected in `proxy_ports`. |
| `tls` | Certificate, key, and optional upstream CA configuration. See the TLS instructions below. |

`interface`, `listen_port`, `proxy_mark`, `ignore_mark`, and `route_table` are accepted for compatibility but ignored. The controller chooses the default IPv4 interface and its internal listener port. The listener port is selected outside `proxy_ports`, without checking whether another process has bound it.

The bridge rule always excludes TCP connections with source or destination port 22, even when port 22 appears in `proxy_ports`.

### Select requests and responses

Each rule requires `target`, `selector`, and `actions`. Set `target` to `Request` or `Response`.

Request selectors support destination `port`, case-sensitive wildcard `path`, exact `method`, and `request_headers`. Response selectors also support `code` and `response_headers`. All supplied conditions must match. An empty selector matches all HTTP exchanges for that target. A path such as `/example` matches that exact path. Use `/example*` for a prefix match.

For each request or response, the proxy selects all matching rules before applying their actions in configuration order. Response selectors inspect the request after request actions and the response before response actions.

### Apply actions

Within each rule, actions run in this order.

1. `abort: true` stops the HTTP exchange without returning a configured HTTP status.
2. `delay` waits for a duration such as `500ms` or `5s`.
3. `replace` changes named fields.
4. `patch` appends request query parameters or headers, or merges a JSON body.

Request replacement supports `path`, `method`, `queries`, `headers`, and `body`. Response replacement supports `code`, `headers`, and `body`. `replace.queries` updates named query parameters. `replace.headers` overwrites named headers.

Use `TEXT` or `BASE64` for replacement body contents. Use `JSON` for a body merge patch. Body patches follow JSON Merge Patch semantics, so the existing body must be valid JSON. Both replacement and merge remove `Content-Length` so the HTTP transport can frame the new body. `update_content_length` is not a supported option, although older examples contain it.

This rule replaces a request body, adds a query parameter, and appends a header.

```yaml
proxy_ports: [8080]
rules:
  - target: Request
    selector:
      path: /example*
      method: POST
    actions:
      replace:
        body:
          contents:
            type: TEXT
            value: '{"message":"Hello"}'
      patch:
        queries:
          - [source, chaos-tproxy]
        headers:
          - [x-chaos-test, enabled]
```

Without `tls`, the proxy tries to parse HTTP and falls back to forwarding raw TCP after an HTTP parse error. HTTP fault rules do not apply to that raw TCP traffic.

### Intercept TLS traffic

Add a TLS certificate and a PEM RSA PKCS#1 private key. Clients must trust the interception certificate, and it must cover the requested server name.

```yaml
proxy_ports: [443]
tls:
  cert_file:
    type: Path
    value: /path/to/server.crt
  key_file:
    type: Path
    value: /path/to/server-rsa.key
```

To trust a private upstream CA, add `tls.ca_file` with the same `type: Path` and `value` shape. A supplied CA bundle replaces the bundled public roots. If omitted, the proxy uses the bundled public roots. `type: Contents` accepts a numeric byte array instead of a file path. PKCS#8 and EC private keys are not supported by the current key parser.

TLS configuration applies to every intercepted port. Use a TLS-only port list for that instance. There is no plain HTTP or raw TCP fallback when `tls` is configured.

## Apply configuration over standard input

Start interactive mode without a configuration file.

```sh
sudo ./tproxy -i -v
```

The controller reads HTTP `PUT` requests from standard input and writes HTTP responses to standard output. The body must be JSON. Logs go to standard error. Each PUT replaces the previous configuration. A positional file takes precedence over `-i`.

For example, send this request with CRLF line endings and a blank line before the body. The body below is exactly 106 bytes, without a trailing newline.

```http
PUT / HTTP/1.1
Host: localhost
Content-Type: application/json
Content-Length: 106

{"proxy_ports":[8080],"rules":[{"target":"Request","selector":{"method":"GET"},"actions":{"delay":"5s"}}]}
```

A successful controller reload returns `HTTP/1.1 200 OK`. Configuration parse errors return 400. Other HTTP methods return 405. A 200 response does not confirm that the proxy subprocess has finished TLS validation or started listening.

To stop interception while keeping the interactive controller alive, send this request. The body is two bytes.

```http
PUT / HTTP/1.1
Host: localhost
Content-Length: 2

{}
```

Press Ctrl-C or send SIGTERM to exit. Use an explicit stop request or signal rather than relying on standard input closing to restore networking.

## Inspect CLI options

```sh
./tproxy --help
```

Use `-v`, `-vv`, or `-vvv` for info, debug, or trace logs. `--proxy` and `--ipc-path` are internal subprocess options. The controller starts that subprocess itself.

## Develop and test

The workspace contains the Rust controller, the HTTP proxy library, and integration tests. `podnetmock/` is a separate Go package for UDP probes and packet capture. See [AGENTS.md](AGENTS.md) for the source map and change constraints.

Run the Rust checks on Linux from the repository root.

```sh
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo +nightly fmt --all -- --check
```

The focused proxy, controller configuration, and interface-selection tests run during `make image`. The ignored HTTP content-length test needs the Python fixture on port 8080. Start the fixture from `tests/` in one terminal.

```sh
cd tests
python3 integrations/checker.py
```

In another terminal, run the test from `tests/` and stop the fixture afterward.

```sh
cd tests
cargo test --locked --release --test integration -- --ignored test_http_action::test_http_content_length_replace
```

The ignored route test deletes routes. Run it only in a disposable Linux environment. For the Go package, install libpcap headers and run `go test ./...` on Linux with packet capture permissions. Its test uses the `lo` interface.

These checks do not verify transparent interception or network restoration. Verify those flows separately in the prepared Linux environment.

`make build` produces `target/debug/chaos-tproxy`. To run that build, use `sudo ./target/debug/chaos-tproxy -v example.yaml`. The current `make run` target points to `target/debug/tproxy`, which Cargo does not produce.
