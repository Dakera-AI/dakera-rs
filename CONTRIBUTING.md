# Contributing to dakera-rs

Thank you for your interest in contributing to the Dakera Rust SDK.

## Development Setup

### Prerequisites

- Rust 1.70 or later (stable toolchain)
- A running Dakera instance for integration tests (optional)

### Building

```sh
# Check the project compiles
cargo check --all-features

# Build the project
cargo build --all-features
```

### Testing

```sh
# Run all tests
cargo test --all-features

# Run tests for a specific module
cargo test --all-features -- grpc
```

### Linting

```sh
# Run clippy
cargo clippy --all-features -- -D warnings

# Check formatting
cargo fmt --check

# Fix formatting
cargo fmt
```

## Pull Request Process

1. Fork the repository and create a feature branch from `main`.
2. Ensure `cargo check --all-features`, `cargo test --all-features`, `cargo clippy --all-features -- -D warnings`, and `cargo fmt --check` all pass.
3. Update documentation and CHANGELOG.md if applicable.
4. Submit a pull request with a clear description of the changes.

## Code Style

- Follow standard Rust conventions and idioms.
- Use `rustfmt` defaults for formatting.
- Write doc comments for all public types and functions.
- Prefer strong typing over stringly-typed APIs.
- Keep `unsafe` usage to an absolute minimum, and document the safety invariants.

## Reporting Issues

Use the [Bug Report](https://github.com/Dakera-AI/dakera-rs/issues/new?template=bug_report.md) template to report bugs. Please include:
- Rust version (`rustc --version`), operating system
- Steps to reproduce the issue
- Expected vs actual behavior

Have a feature idea? Use the [Feature Request](https://github.com/Dakera-AI/dakera-rs/issues/new?template=feature_request.md) template.

## Security Vulnerabilities

**Do not open public issues for security vulnerabilities.** See [SECURITY.md](.github/SECURITY.md) for responsible disclosure instructions — email security@dakera.ai.

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
