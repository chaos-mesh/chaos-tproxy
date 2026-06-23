# chaos-tproxy Development Guidelines

## Library-First Principle

Every new feature MUST first be implemented as a Go package under `pkg/`.
Application entrypoints, including `cmd/chaos-tproxy`, MUST delegate feature
logic to that package instead of owning the behavior directly.

Each feature package MUST:

- Expose a small, explicit public API for the behavior it owns
- Keep command-line parsing, process I/O, and other application concerns out of
  the package core
- Keep dependencies minimal and scoped to the package's responsibility
- Be usable from tests without invoking the CLI binary

## CLI Interface Mandate

Every feature MUST expose its functionality through a command-line interface.
The CLI is the supported operational entrypoint for users and automation.

Each feature CLI MUST:

- Accept text as input (via stdin, arguments, or files)
- Produce text as output (via stdout)
- Support JSON format for structured data exchange
- Return non-zero exit codes for failed operations
- Delegate feature behavior to the corresponding `pkg/` package

## Test-First Imperative

This is NON-NEGOTIABLE: All implementation MUST follow strict Test-Driven Development.
No implementation code shall be written before:

1. Unit tests or integration tests are written
2. Tests are validated and approved by the user
3. Tests are confirmed to FAIL (Red phase)

Integration tests MUST live under `tests/` and use the Ginkgo framework.
They SHOULD be started through Makefile targets so local development and CI use
the same entrypoints. Feature work is not complete until the relevant Makefile
test target passes.
