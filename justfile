# Common development tasks. Install just: https://github.com/casey/just

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

# Lint with warnings as errors (same as CI)
clippy:
    cargo clippy --workspace -- -D warnings

# Run all tests
test:
    cargo test --workspace

# Run everything CI runs (catch failures before pushing)
ci: fmt-check clippy test

# Build API docs for the main crates
doc:
    cargo doc --no-deps -p amigo_core -p amigo_render -p amigo_audio -p amigo_input -p amigo_engine
