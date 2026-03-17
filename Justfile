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

# Automated release: bump version, generate changelog, tag, and push
release-auto version:
    #!/usr/bin/env bash
    set -euo pipefail
    VERSION="{{version}}"
    # Strip leading 'v' for semver if present
    SEMVER="${VERSION#v}"
    TAG="v${SEMVER}"

    # Validate semver format
    if ! echo "$SEMVER" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+'; then
        echo "Error: version must be semver (e.g., 0.2.0 or v0.2.0)"
        exit 1
    fi

    # Ensure working tree is clean
    if [ -n "$(git status --porcelain)" ]; then
        echo "Error: working tree is not clean. Commit or stash changes first."
        exit 1
    fi

    # Ensure we're on main
    BRANCH=$(git branch --show-current)
    if [ "$BRANCH" != "main" ]; then
        echo "Error: must be on main branch (currently on $BRANCH)"
        exit 1
    fi

    echo "Releasing $TAG..."

    # Update versions in all workspace Cargo.toml files
    for toml in shared/Cargo.toml crates/*/Cargo.toml; do
        sed -i '' "s/^version = \".*\"/version = \"${SEMVER}\"/" "$toml"
    done

    # Generate changelog
    git cliff --tag "$TAG" --output CHANGELOG.md

    # Commit, tag, push
    git add -A
    git commit -m "chore: release ${TAG}"
    git tag -a "$TAG" -m "Release ${TAG}"
    git push origin main "$TAG"

    echo "Released $TAG"

# Start docs dev server
docs-dev:
    cd docs && npm run dev

# Build docs
docs-build:
    cd docs && npm run build

# Preview built docs locally
docs-preview:
    cd docs && npm run preview
