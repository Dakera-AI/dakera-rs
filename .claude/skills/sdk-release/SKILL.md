---
name: sdk-release
description: Release the Dakera Rust SDK. Use when publishing a new version to crates.io.
disable-model-invocation: true
allowed-tools: Bash(gh *) Bash(cargo *)
---

## Rust SDK Release

### Pre-release checks
```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

### Version bump
Update version in `Cargo.toml` under `[package].version`.

### Release process
1. Update `CHANGELOG.md`
2. Commit: `git commit -m "chore: bump to vX.Y.Z"`
3. Tag: `git tag vX.Y.Z`
4. Push: `git push origin main --tags`
5. Release workflow auto-publishes to crates.io

### Batching rules
- All 4 SDKs (py, js, rs, go) sync in a single coordinated batch
- Do NOT release for a single trivial change — batch until 2+ changes or security fix
