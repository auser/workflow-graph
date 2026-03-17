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

# Bump version in all workspace Cargo.toml files
bump-versions version:
    #!/usr/bin/env bash
    set -euo pipefail
    awk -v ver="{{version}}" '
        /^\[workspace\.package\]/ { in_ws=1 }
        /^\[/ && !/^\[workspace\.package\]/ { in_ws=0 }
        in_ws && /^version = "/ { print "version = \"" ver "\""; next }
        { print }
    ' Cargo.toml > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
    echo "Bumped workspace version to {{version}}"

# Cut a release with automatic version bump (based on conventional commits)
release-auto:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "==> Preparing automatic release"

    # 1. Ensure working tree is clean
    if [ -n "$(git status --porcelain)" ]; then
        echo "Error: working tree is not clean. Commit or stash changes first."
        exit 1
    fi

    # 2. Quality gates
    cargo fmt --all
    cargo clippy --fix --allow-dirty --workspace --all-targets -- -D warnings
    cargo clippy --workspace --all-targets -- -D warnings
    cargo nextest run --workspace

    # 3. Determine next version from conventional commits
    NEXT_VERSION=$(git-cliff --bumped-version | sed 's/^v//')
    TAG="v${NEXT_VERSION}"
    echo "==> Auto-detected next version: $NEXT_VERSION (tag: $TAG)"

    # 4. Bump all workspace versions and update lockfile
    just bump-versions "$NEXT_VERSION"
    cargo update --workspace

    # 5. Commit version bump + any fmt/clippy fixes
    if ! git diff --quiet || [ -n "$(git ls-files --others --exclude-standard)" ]; then
        git add -u
        git commit -m "chore: bump workspace version to $NEXT_VERSION"
    fi

    # 6. Generate changelog
    touch CHANGELOG.md
    if ! grep -q "^## \[$NEXT_VERSION\]" CHANGELOG.md 2>/dev/null; then
        git-cliff --tag "$TAG" --output CHANGELOG.md
        git add CHANGELOG.md
        git commit -m "chore: add changelog for $TAG"
    fi

    # 7. Verify changelog & crate versions match
    scripts/verify-release-version.sh --version "$NEXT_VERSION"

    # 8. Tag and push
    git push
    if ! git tag -l "$TAG" | grep -q .; then
        git tag -a "$TAG" -m "Release $TAG"
    fi
    git push origin "$TAG"
    echo "==> $TAG released and pushed."

# Start docs dev server
docs-dev:
    cd docs && npm run dev

# Build docs
docs-build:
    cd docs && npm run build

# Preview built docs locally
docs-preview:
    cd docs && npm run preview
