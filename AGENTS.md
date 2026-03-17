# Agent Guidelines

## Quality Gates

Before committing any code changes, ensure:

1. **`cargo fmt --all`** — all Rust code must be formatted
2. **`cargo clippy --workspace --all-targets -- -D warnings`** — zero clippy warnings
3. **`cargo test --workspace`** — all 38+ tests must pass
4. **`cargo check --workspace`** — workspace compiles cleanly

These are enforced by `just release-auto` and must pass before any release.

## Code Style

- Rust edition 2024, strict clippy
- Use `#[allow(clippy::too_many_arguments)]` only on WASM-exported functions where the signature is dictated by the JS API
- Prefer `thiserror` for error types, not ad-hoc strings
- All public Rust APIs should have doc comments
- TypeScript packages use `strict: true` — no `any` types

## Architecture Rules

- **WASM crate** (`crates/web/`) — no `Closure::forget()` for canvas event listeners; store closures in `StoredListener` and clean up in `destroy()`
- **Queue traits** — all queue operations return `Result<T, QueueError>`; never silently swallow errors
- **Server** — CORS is configurable via `CORS_ORIGINS` env var; don't hard-code permissive CORS
- **Worker SDK** — must handle SIGTERM/SIGINT for graceful shutdown
- **npm packages** — `main`/`types` must point to `dist/` compiled output, never raw `.ts`/`.tsx` source

## Testing

- Unit tests in `#[cfg(test)]` modules alongside source
- Integration tests in `crates/queue/tests/integration.rs`
- Performance benchmarks in `crates/queue/tests/performance.rs`
- YAML parser edge cases in `shared/src/yaml.rs`

## Release Process

```bash
just release-auto
```

This runs fmt, clippy, tests, version bump, changelog generation, and tag push.
