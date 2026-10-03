# Common development tasks. Install just: https://github.com/casey/just
#
# CI (.github/workflows/ci.yml) runs these recipes, not its own copies of the
# commands, so `just ci` reproduces CI exactly. The feature-matrix recipes need
# cargo-hack (`cargo install --locked cargo-hack`).

# List available recipes
default:
    @just --list

# Type-check the whole workspace (crates, tools, examples)
check:
    cargo check --workspace

# Check formatting
fmt-check:
    cargo fmt --all -- --check

# Format all code
fmt:
    cargo fmt --all

# Lint every target (lib, bins, tests, examples) with warnings as errors
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Run all tests
test:
    cargo test --workspace

# Lint every crate once per feature, alone, and once with all features. Found
# by cargo-hack from the manifests, so a new feature is covered without editing
# this file. Features a dependent enables are otherwise never checked alone.
clippy-features:
    cargo hack clippy --workspace --each-feature --all-targets -- -D warnings
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# Tests behind feature flags. `--all-features` covers every gated test; the
# single-feature runs guard features whose tests change behaviour on their own.
test-features:
    cargo test --workspace --all-features
    cargo test -p amigo_core --features async_tasks
    cargo test -p amigo_core --features system_graph
    cargo test -p amigo_engine --features api

# Doc tests for every library crate
test-doc:
    cargo test --workspace --doc

# Run everything CI runs (catch failures before pushing)
ci: fmt-check check clippy clippy-features test test-features test-doc doc

# Build API docs for every library crate, failing on doc warnings
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --lib
