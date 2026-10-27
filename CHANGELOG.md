# Changelog

Notable changes to chaos-tproxy are recorded here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.7] - 2026-10-27

### Changed

- Use Rust 1.99.1 for source and Docker builds. [#81](https://github.com/chaos-mesh/chaos-tproxy/pull/81)

### Fixed

- Wait for proxy configuration validation and listener readiness before reporting successful startup. Roll back networking when startup fails. [#77](https://github.com/chaos-mesh/chaos-tproxy/pull/77)
- Preserve unrelated NAT rules and restore displaced routes on stop. Keep routes added or changed during interception, and restore original routes alongside unchanged backup routes. [#77](https://github.com/chaos-mesh/chaos-tproxy/pull/77), [#79](https://github.com/chaos-mesh/chaos-tproxy/pull/79)
- Preserve repeated headers in HTTP patches and unrelated repeated query parameters in query replacements. [#79](https://github.com/chaos-mesh/chaos-tproxy/pull/79)
- Stop interception when interactive standard input reaches EOF. Preserve queued configuration requests across `Connection: close` and process buffered requests before shutdown. [#79](https://github.com/chaos-mesh/chaos-tproxy/pull/79), [#80](https://github.com/chaos-mesh/chaos-tproxy/pull/80)
- Cancel gateway ping tasks on reload, stop, and startup failure. [#80](https://github.com/chaos-mesh/chaos-tproxy/pull/80)
- Reuse the UDP probe sender socket, match the key supplied to the monitor, and close packet captures before reporting completion. [#80](https://github.com/chaos-mesh/chaos-tproxy/pull/80)
- Return a successful exit status for `--help` and `--version`, and fix `make run` to invoke the built binary. [#80](https://github.com/chaos-mesh/chaos-tproxy/pull/80)

## [0.5.6] - 2026-10-13

### Fixed

- Allow HTTPChaos pause and recovery after the controller moves the original interface's IPv4 address. [#76](https://github.com/chaos-mesh/chaos-tproxy/pull/76)
- Preserve Client and Server role filtering on rule reloads when a secondary IPv4 interface is present. [#76](https://github.com/chaos-mesh/chaos-tproxy/pull/76)

[0.5.7]: https://github.com/chaos-mesh/chaos-tproxy/compare/v0.5.6...HEAD
[0.5.6]: https://github.com/chaos-mesh/chaos-tproxy/compare/v0.5.5...v0.5.6
