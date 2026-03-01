_default:
    just --list -u

alias t := test
alias f := format
alias l := lint
alias b := build
alias x := xtask
alias tsc := typecheck
alias bench := benchmark

# Setup development environment
setup:
    # Setup Node.js environment
    corepack enable
    corepack prepare --activate
    yarn
    yarn lefthook install

    # Setup xtask
    yarn workspace xtask run setup

    # Run build
    just build

# Test all files
test: test-rs test-js

# Test JS files
test-js: build-napi build-js
    yarn vitest run

# Test Rust files
test-rs:
    cargo test --workspace --no-fail-fast --all-features

# Format all files
format: format-rs format-js format-toml

# Format Rust files
format-rs:
    cargo fmt --all

# Format JS files via oxfmt
format-js:
    yarn oxfmt

# Format TOML files via taplo
format-toml:
    yarn taplo format

# Lint all files
lint: lint-rs lint-js

# Lint JS files via oxlint
lint-js:
    yarn oxlint --type-aware

# Lint Rust files via Clippy
lint-rs:
    cargo clippy --workspace

# Typechecking with TSC
typecheck:
    yarn workspaces foreach -Apt run typecheck

# Build as release mode
build: build-rs build-napi build-js

# Build NAPI modules
build-napi:
    yarn workspaces foreach -Apt --include='@wvb/*' run build-napi

# Build Rust workspaces
build-rs:
    cargo build --workspace

# Build JS packages
build-js:
    yarn workspaces foreach -Apt --include='@wvb/*' run build

# Run benchmarks
benchmark: build
    yarn workspaces foreach -Apt --include='@benchmark/*' run bench

# Start website dev server
website:
    yarn workspace wvb-website run typegen
    yarn workspace wvb-website run dev

# Run xtask
xtask *ARGS:
    node ./xtask/cli.ts {{ ARGS }}

git_current_branch := shell('git rev-parse --abbrev-ref HEAD')

# Prerelease
prerelease:
    #!/usr/bin/env bash
    if [ "{{ git_current_branch}}" != "main" ]; then \
      echo "prerelease script must be run in \"main\" branch"; \
      exit 1; \
    fi
    git tag -a prerelease -m "prerelease" --force
    git push origin prerelease --force

# Release
release:
    #!/usr/bin/env bash
    if [ "{{ git_current_branch}}" != "main" ]; then \
      echo "release script must be run in \"main\" branch"; \
      exit 1; \
    fi
    git tag -a release -m "release" --force
    git push origin release --force
