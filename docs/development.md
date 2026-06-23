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

CLI entrypoint code MUST live under `pkg/cli`.

The `pkg/cli` package is an application entry layer. It MAY depend on feature
packages under `pkg/`, but feature packages MUST NOT depend on `pkg/cli`.
In application code, `pkg/cli` MUST only be imported by the main package.
Integration tests SHOULD exercise CLI behavior through the compiled command
instead of importing `pkg/cli` directly.

Each feature CLI MUST:

- Accept text as input (via stdin, arguments, or files)
- Produce text as output (via stdout)
- Support JSON format for structured data exchange
- Return non-zero exit codes for failed operations
- Delegate feature behavior to the corresponding `pkg/` package

## Test-First Imperative

This is NON-NEGOTIABLE: All implementation MUST follow strict Test-Driven Development.
The first TDD step is to determine the interface contract the tests will
exercise. Before writing tests, define the target package API and, when
applicable, the CLI command contract: names, inputs, outputs, errors, and
behavior boundaries. Tests MUST be written against this agreed interface, not
against an implementation invented during the test-writing step.

No implementation code shall be written before:

1. The interface contract is determined and approved by the user
2. Unit tests or integration tests are written against that interface
3. Tests are validated and approved by the user
4. Tests are confirmed to FAIL (Red phase)

Integration tests MUST live under `tests/integration/<package>/` and use the
Ginkgo framework. For example, integration tests for `pkg/runtime` MUST live
under `tests/integration/runtime/`.

Package integration tests MUST directly import and exercise the corresponding
`pkg/<package>` package. They MUST NOT invoke `cmd/chaos-tproxy` or depend on
the compiled CLI binary for package behavior coverage. CLI behavior tests are a
separate concern and should exercise the compiled command entrypoint.

Helper functions for package integration tests MUST stay in the same
`tests/integration/<package>/` package unless they are intentionally shared by
multiple integration packages.

Every package integration suite MUST have a Makefile target named
`test-<package>` that runs that package's Ginkgo suite directly. For example,
`pkg/runtime` MUST be tested through `make test-runtime`, which runs
`$(GINKGO) -r ./tests/integration/runtime`.

Package integration tests MUST be started through their `test-<package>`
Makefile target so local development and CI use the same entrypoint. Feature
work is not complete until the relevant `test-<package>` target passes.
