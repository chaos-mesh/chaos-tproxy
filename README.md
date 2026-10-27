# chaos-tproxy

chaos-tproxy is a transparent HTTP fault injection proxy for Linux. It uses TPROXY to intercept IPv4 TCP traffic without application proxy settings. Rules can abort or delay HTTP requests and responses, replace fields, and merge JSON bodies.

## Requirements

Run chaos-tproxy as root in a disposable Linux VM or container with an IPv4 default interface. The controller modifies interface addresses, bridges, routes, firewall rules, and resolver files. Cleanup removes its own ebtables NAT jump and chain and restores routes displaced by setup. Routes added or changed during interception take precedence over saved routes. Existing backup routes do not prevent restoration. Do not run it in the host network namespace.

The following commands must be available on `PATH`.

```sh
command -v ip sysctl arp iptables ebtables ebtables-legacy
```

Install `iproute2` for `ip` and `net-tools` for `arp`. The controller uses both `ebtables` and `ebtables-legacy`. The BROUTING rule requires the legacy implementation.

The kernel must support bridges, network namespaces, policy routing, and TPROXY. Relevant modules include `ebtables`, `ebtable_broute`, `iptable_mangle`, `xt_TPROXY`, and `xt_socket`. These features can also be built into the kernel.

Use a DNS resolver reachable after network setup. Loopback-only resolvers can become unreachable when the controller changes networking.

## Installation

Download the archive for your Linux architecture from [GitHub releases](https://github.com/chaos-mesh/chaos-tproxy/releases). The archives `tproxy-x86_64.tar.gz` and `tproxy-aarch64.tar.gz` contain a binary named `tproxy`.

To build from source, run the following command on Linux from the repository root.

```sh
cargo build --locked --release --workspace
```

The executable is `target/release/chaos-tproxy`. The repository selects Rust 1.99.1 through `rust-toolchain`.

To build with Docker and copy the executable to `./tproxy`, run the following command.

```sh
make release
```

The Dockerfile uses Rust 1.99.1 and runs the nonignored proxy and controller library tests before building. The image provides a build environment. It does not include all runtime networking tools. `make release` does not publish a GitHub release.

The examples below use `./tproxy`. For a native source build, use `./target/release/chaos-tproxy` instead.

## Usage

Save the following configuration as `example.yaml`. It delays GET requests matching `/example*` on destination port 8080 by five seconds.

```yaml
proxy_ports: [8080]
rules:
  - target: Request
    selector:
      path: /example*
      method: GET
    actions:
      delay: 5s
```

Start the controller in the prepared Linux environment.

```sh
sudo ./tproxy -v example.yaml
```

Send a matching request through the intercepted interface to an HTTP server on port 8080. Loopback requests do not exercise this network path. Press Ctrl-C or send SIGTERM to stop the controller and trigger network cleanup.

Use `./tproxy --help` to list CLI options. The flags `-v`, `-vv`, and `-vvv` enable info, debug, and trace logs. `--proxy` and `--ipc-path` are internal options for the subprocess that the controller starts.

## Configuration

File mode accepts `.json` and `.yaml` files. The `.yml` extension is not supported. See [configuration examples](config-examples/) for additional configurations.

The controller accepts the following fields.

| Field | Behavior |
| --- | --- |
| `proxy_ports` | Destination TCP ports to intercept. An omitted or `null` value stops interception. An empty array attempts network setup and does not stop interception. |
| `rules` | HTTP fault rules. Defaults to an empty list. |
| `role` | `Client` matches the connection source against the default interface's IPv4 addresses. `Server` matches the destination. An omitted value applies rules in both directions. |
| `safe_mode` | Defaults to `false`. When enabled, bypasses TCP traffic for the default interface's IPv4 subnet when the source or destination port is between 1 and 1025. Selected `proxy_ports` can be bypassed. |
| `tls` | Interception certificate, private key, and optional upstream CA bundle. See [TLS](#tls). |

The fields `interface`, `listen_port`, `proxy_mark`, `ignore_mark`, and `route_table` are accepted for compatibility but have no effect. The controller uses the default IPv4 interface and selects a listener port outside `proxy_ports`. It does not check whether that listener port is already in use.

TCP connections with source or destination port 22 bypass interception, including when `proxy_ports` contains port 22.

### Selectors

Each rule requires `target`, `selector`, and `actions`. The target is `Request` or `Response`.

Request selectors support destination `port`, case-sensitive wildcard `path`, exact `method`, and `request_headers`. Response selectors also support `code` and `response_headers`. All applicable conditions must match. An empty selector matches every HTTP exchange for its target.

The path `/example` matches that exact path. The path `/example*` matches a prefix. Header selectors match the specified values.

The proxy selects all matching rules before applying actions in configuration order. Response selectors inspect the request after request actions and the response before response actions.

### Actions

Actions within a rule execute in the following order.

1. `abort: true` terminates the HTTP exchange without a configured HTTP status.
2. `delay` waits for the specified duration, such as `500ms` or `5s`.
3. `replace` changes the specified fields.
4. `patch` appends request query parameters or headers, or merges a JSON body.

Request replacement supports `path`, `method`, `queries`, `headers`, and `body`. Response replacement supports `code`, `headers`, and `body`. Query replacement updates named parameters. Header replacement overwrites named headers.

Body replacement accepts `TEXT` or `BASE64` contents. Body patches accept `JSON` contents and use JSON Merge Patch semantics. For a merge patch, the existing body must be valid JSON. Both operations remove `Content-Length` so the HTTP transport can frame the new body. The `update_content_length` option is not supported.

The following rule replaces a request body, adds a query parameter, and appends a header.

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

Without TLS configuration, an HTTP parse error causes the proxy to forward the connection as raw TCP. HTTP fault rules do not apply to raw TCP traffic.

### TLS

TLS interception requires a certificate and a PEM RSA PKCS#1 private key. Clients must trust the certificate, and the certificate must cover the requested server name.

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

The optional `tls.ca_file` field uses the same file format and supplies trusted upstream CA certificates. A supplied bundle replaces the bundled public roots. Without `ca_file`, the proxy uses the bundled public roots. File values also accept `type: Contents` with a numeric byte array. PKCS#8 and EC private keys are not supported.

TLS configuration applies to every intercepted port. Configure only TLS ports for that instance. Plain HTTP and raw TCP fallback are unavailable when TLS is configured.

## Interactive configuration

Start interactive mode without a configuration file.

```sh
sudo ./tproxy -i -v
```

The controller reads HTTP `PUT` requests from standard input and writes HTTP responses to standard output. Request bodies must be JSON. Each accepted PUT applies a complete replacement configuration. Logs go to standard error. A positional configuration file takes precedence over `-i`.

The following request applies a five-second delay to GET requests on port 8080. Send it with CRLF line endings. The body is 106 bytes without a trailing newline.

```http
PUT / HTTP/1.1
Host: localhost
Content-Type: application/json
Content-Length: 106

{"proxy_ports":[8080],"rules":[{"target":"Request","selector":{"method":"GET"},"actions":{"delay":"5s"}}]}
```

A successful reload returns `200 OK` after the proxy subprocess validates its configuration, including TLS files, and binds its listener. Body read, configuration parsing, or conversion errors return `400 Bad Request`. Startup or cleanup failures return `500 Internal Server Error` with an error body. Startup waits at most ten seconds for readiness, then stops the child and rolls back networking on failure. Other methods return `405 Method Not Allowed`.

To stop interception without exiting interactive mode, send a PUT with body `{}` and `Content-Length: 2`. Close standard input, press Ctrl-C, or send SIGTERM to exit and trigger cleanup.

## Development

The Rust workspace contains the controller, proxy library, and integration tests. `podnetmock/` is a separate Go package for UDP probes and packet capture.

CI uses Rust 1.99.1 to build, test, lint, and format. Run `cargo fmt --all -- --check` to check formatting.

To use the development container, open the repository in VS Code with the Dev Containers extension and select **Dev Containers: Rebuild and Reopen in Container**. It provides Debian Trixie, Rust 1.99.1 with rustfmt and Clippy, Go 1.27.3, libpcap headers, and the networking tools listed above. Cargo downloads and Go module and build caches use shared Docker volumes across container rebuilds.

The development container runs as root in privileged mode. Network interception still requires the Docker host's Linux kernel to provide the required networking modules. Run interception only inside the disposable container, using its own network namespace.

See [AGENTS.md](AGENTS.md) for AI agent instructions, the repository map, and verification commands. Rust checks require Linux. Library tests and Docker builds do not verify network interception or restoration.
