# Default port (server will auto-find next available if taken)
port := "3000"

# ─────────────────────────────────────────────────────────────────────────────
# Justfile — socialsite monorepo task runner
# Usage: just <recipe>   (install: brew install just)
# ─────────────────────────────────────────────────────────────────────────────

# Default: list all recipes
default:
    @just --list


# Build WASM frontend
build-wasm:
    wasm-pack build crates/web --target web --no-typescript

# Build server
build-server:
    cargo build -p workflow-graph-server

# Build everything
build: build-wasm build-server

# Run the server (auto-finds available port if taken)
serve:
    PORT={{port}} cargo run -p workflow-graph-server

# Run server in release mode
serve-release: build-wasm
    PORT={{port}} cargo run -p workflow-graph-server --release

# Watch server for changes and auto-restart
watch:
    PORT={{port}} cargo watch -x 'run -p workflow-graph-server' -w crates/server/src -w shared/src -w workflows

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

# Generate changelog for a version (e.g., just changelog v0.2.0)
changelog version="":
    #!/usr/bin/env bash
    if [ -z "{{version}}" ]; then
        git cliff --unreleased
    else
        git cliff --tag "{{version}}"
    fi

# Cut a release with automatic version bump (based on conventional commits)
release-auto:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "==> Preparing automatic release"
    # 1. Quality gates — auto-fix fmt and clippy, then test
    cargo fmt --all
    cargo clippy --fix --allow-dirty --workspace --all-targets -- -D warnings
    cargo clippy --workspace --all-targets -- -D warnings
    cargo nextest run --workspace
    # 2. Determine next version from conventional commits
    NEXT_VERSION=$(git-cliff --bumped-version | sed 's/^v//')
    echo "==> Auto-detected next version: $NEXT_VERSION"
    # 3. Bump all workspace versions
    just bump-versions "$NEXT_VERSION"
    # 4. Commit the version bump + any fmt/clippy fixes + updated Cargo.lock
    if ! git diff --quiet; then
        git add -u
        git commit -m "chore: bump workspace version to $NEXT_VERSION"
    fi
    # 5. Generate changelog entry from conventional commits (via git-cliff)
    if ! grep -q "^## \[$NEXT_VERSION\]" CHANGELOG.md; then
        git-cliff --tag "v$NEXT_VERSION" --unreleased --prepend CHANGELOG.md
        git add CHANGELOG.md
        git commit -m "chore: add changelog entry for v$NEXT_VERSION"
    fi
    # 6. Verify changelog & crate versions match
    scripts/verify-release-version.sh --version "$NEXT_VERSION"
    # 7. Push branch commits, tag, and push tag (triggers .github/workflows/release.yml)
    git push
    if ! git tag -l "v$NEXT_VERSION" | grep -q .; then
        git tag "v$NEXT_VERSION"
    fi
    git push origin "v$NEXT_VERSION"
    echo "==> Tag v$NEXT_VERSION pushed. Release workflow will build and publish."

# Start docs dev server
docs-dev:
    cd docs && npm run dev

# Build docs
docs-build:
    cd docs && npm run build

# Preview built docs locally
docs-preview:
    cd docs && npm run preview
