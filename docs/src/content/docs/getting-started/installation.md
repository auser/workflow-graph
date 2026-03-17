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
npm install @auser/workflow-graph-web      # WASM + Canvas renderer (includes .wasm binary)
npm install @auser/workflow-graph-react    # React component (peer dep: @auser/workflow-graph-web)
npm install @auser/workflow-graph-client   # TypeScript REST API client
```

The `@auser/workflow-graph-web` package bundles the compiled WASM binary in its `wasm/` directory. You need to serve this file and tell the loader where to find it:

```typescript
import { setWasmUrl } from '@auser/workflow-graph-web';

// Point to where you serve the .wasm file (e.g., public/ directory)
setWasmUrl('/workflow_graph_web_bg.wasm');
```

Copy the WASM file to your static assets:

```bash
cp node_modules/@auser/workflow-graph-web/wasm/workflow_graph_web_bg.wasm public/
```

:::tip
See the [examples directory](https://github.com/auser/workflow-graph/tree/main/examples) for complete Vite setups (vanilla JS and React) that handle WASM serving automatically.
:::

## What's Included

| Package | Size | Features |
|---------|------|----------|
| `@auser/workflow-graph-web` | ~100KB gzipped | Renderer, theming (dark/light/high-contrast), minimap, i18n, touch support, a11y |
| `@auser/workflow-graph-react` | ~3KB | React wrapper with ref API, loading skeleton, SSR guard, error boundary |
| `@auser/workflow-graph-client` | ~2KB | Typed REST client with `WorkflowApiError`, log streaming |

All packages ship with TypeScript declarations (`strict: true`) and ES module output.
