# Contributing to Sniplic Core

Thank you for your interest in contributing to Sniplic! This is an open-source, community-driven NLE video engine.

## Guidelines

- **Language**: Core logic is Rust only (`sniplic-core`). Node.js/TypeScript is exclusively used for the wrapper and tests in `bindings/node`. Never write business logic in JavaScript.
- **Licence**: Contributions must be made under **MIT OR Apache-2.0** (`LICENSE-MIT`, `LICENSE-APACHE`). 
- **Clean-room**: Do not copy code, logic, or proprietary models from commercial NLEs (such as Premiere Pro, DaVinci Resolve, or Final Cut). Match behavior and look by observation and public specs. 
- **Never crash**: Non-test Rust code must not panic. No `unwrap()`, `expect()`, `panic!()`, `unreachable!()`, `todo!()`, or `unimplemented!()`. Do not use `unsafe` blocks unless strictly interfacing with specific FFI layers and justified with a safety comment. Errors must go through `AppResult` and `?`. Input-derived indices, frames, and sizes must be bounds-checked. Every crash fix ships with a regression test.
- **Commands, not handlers**: New features should be implemented as core engine modules/methods in Rust, heavily tested, and then safely exposed via NAPI-RS. 
- **Tests**: Required for every logic change. Code that modifies the timeline, gapless logic, clipping, splitting, or ripple effects must have behavior tests. FFmpeg `filter_complex` builders must have string-matching tests to prevent regression in the export pipeline.
- **Style**: Always run `cargo fmt` and `cargo clippy -- -D warnings`. Match the surrounding code style. Comments should explain *why* something is done, not *what* is done.
- **Commits**: Small, focused, with a clear subject line following conventional commits (e.g., `feat:`, `fix:`, `docs:`, `chore:`).

## Adding a New Feature

1. **Rust Core First**: The algorithm goes in the lowest crate/module that fits (e.g., `timeline`, `ffmpeg`, `audio`), with unit tests. It must operate on the native `Project` structs in memory.
2. **Deterministic Behavior**: Video and audio functions must be deterministic (e.g., gapless timeline evaluation, track sorting).
3. **Expose to NAPI**: Once the core Rust logic is solid, expose it in `bindings/node/src/lib.rs` using the `#[napi]` macro. Ensure asynchronous callbacks use the `tokio::spawn` pattern for Threadsafe Functions.
4. **Docs**: Run cargo docs or update the README if adding a major public feature.

## Development Workflow

To verify your changes before submitting a PR:
```bash
# Check the pure Rust workspace
cargo check --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings

# Build the Node bindings
cd bindings/node
npm install
npm run build
```
