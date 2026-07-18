_default:
    just --list -u

alias t := test
alias f := format
alias l := lint
alias b := build
alias x := xtask
alias tsc := typecheck

# Setup development environment
setup:
    # Setup Node.js environment
    corepack enable
    corepack prepare --activate
    yarn
    yarn lefthook install

    # Run build
    just build

# Test all files
test: test-rs test-js test-ffi test-deno test-e2e test-e2e-ffi

# Test Rust files
test-rs:
    cargo test --workspace --no-fail-fast --all-features

# Test JS files
test-js: build-napi build-js
    yarn vitest run --config vitest.workspace.ts

# Test FFI
test-ffi: build-ffi
    yarn workspace wvb-ffi run test-ffi

# Test E2E
test-e2e:
    yarn vitest run --config vitest.e2e.workspace.ts

# Test FFI-E2E
test-e2e-ffi:
    yarn workspace wvb-ffi run e2e-ffi

# Test deno files
[working-directory: 'packages']
test-deno: build-deno
    deno task test

# Format all files
format: format-rs format-js format-toml

# Format Rust files
format-rs:
    cargo fmt --all

# Format JS files via biome
format-js:
    yarn biome format --write

# Format TOML files via taplo
format-toml:
    yarn taplo format

# Lint all files
lint: lint-rs lint-js

# Lint JS files via biome
lint-js:
    yarn biome check

# Lint Rust files via Clippy
lint-rs:
    cargo clippy --workspace

typecheck: typecheck-js typecheck-deno

# Typecheck
typecheck-js:
    yarn workspaces foreach -Apt run typecheck

# Typecheck deno files
[working-directory: 'packages']
typecheck-deno:
    deno task check

# Build as release mode
build: build-rs build-napi build-js build-ffi build-deno

# Build NAPI modules
build-napi:
    yarn workspaces foreach -Apt --include='@wvb/*' run build-napi

# Build Rust workspaces
build-rs:
    cargo build --workspace --release --exclude=wvb-ffi --exclude=wvb-deno

# Build JS packages
build-js:
    yarn workspaces foreach -Apt --include='@wvb/*' run build

# Build FFI packages
build-ffi:
    yarn workspaces foreach -Apt run build-ffi

# Build Deno crate
build-deno:
    cargo build -p wvb-deno --release

# Run xtask
xtask *ARGS:
    node ./xtask/cli.ts {{ ARGS }}
