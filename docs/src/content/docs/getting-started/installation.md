---
title: Installation
description: Prerequisites and build instructions for workflow-graph
---

## Prerequisites

You need the Rust toolchain with WebAssembly support:

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add WASM target
rustup target add wasm32-unknown-unknown

# Install build tools
cargo install wasm-pack just
```

## Build

```bash
# Build WASM frontend + start server
just dev

# Or separately:
just build-wasm      # Build WASM
just serve           # Start server (auto-finds port if 3000 is taken)
```

## Development with Auto-Reload

```bash
just watch            # cargo-watch restarts server on changes
just watch-all        # Also rebuilds WASM on web crate changes
```

## NPM Packages

If you're integrating via JavaScript/TypeScript, install the packages you need:

```bash
npm install @workflow-graph/web      # WASM wrapper
npm install @workflow-graph/react    # React component
npm install @workflow-graph/client   # TypeScript API client
```
