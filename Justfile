# Default port (server will auto-find next available if taken)
port := "3000"

# Build WASM frontend
build-wasm:
    wasm-pack build crates/web --target web --no-typescript

# Build server
build-server:
    cargo build -p github-graph-server

# Build everything
build: build-wasm build-server

# Run the server (auto-finds available port if taken)
serve:
    PORT={{port}} cargo run -p github-graph-server

# Run server in release mode
serve-release: build-wasm
    PORT={{port}} cargo run -p github-graph-server --release

# Watch server for changes and auto-restart
watch:
    PORT={{port}} cargo watch -x 'run -p github-graph-server' -w crates/server/src -w shared/src -w workflows

# Watch everything: rebuild WASM on web changes, restart server on server changes
watch-all:
    #!/usr/bin/env bash
    just build-wasm
    just watch &
    cargo watch -s 'just build-wasm' -w crates/web/src --no-restart &
    wait

# Check all crates compile
check:
    cargo check --workspace

# Run all tests
test:
    cargo test --workspace

# Clean build artifacts
clean:
    cargo clean
    rm -rf crates/web/pkg

# Rebuild WASM and start server
dev: build-wasm serve

# Trigger sample workflow run via API
run-sample port="3000":
    curl -s -X POST http://localhost:{{port}}/api/workflows/ci-1/run

# Get current workflow status
status port="3000":
    curl -s http://localhost:{{port}}/api/workflows/ci-1/status | python3 -m json.tool

# List all workflows
list port="3000":
    curl -s http://localhost:{{port}}/api/workflows | python3 -m json.tool
