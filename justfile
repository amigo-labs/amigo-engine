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

# Per-crate feature-flag lints/tests, mirroring CI's matrix
clippy-features:
    cargo clippy -p amigo_core --features async_tasks -- -D warnings
    cargo clippy -p amigo_core --features system_graph -- -D warnings
    cargo clippy -p amigo_core --all-features -- -D warnings
    cargo clippy -p amigo_assets --features asset_streaming -- -D warnings
    cargo clippy -p amigo_render --features asset_streaming -- -D warnings
    cargo clippy -p amigo_net --features rollback_net -- -D warnings
    cargo clippy -p amigo_audio --features audio_graph -- -D warnings

test-features:
    cargo test -p amigo_core --features async_tasks
    cargo test -p amigo_core --features system_graph
    cargo test -p amigo_core --all-features
    cargo test -p amigo_assets --features asset_streaming
    cargo test -p amigo_render --features asset_streaming
    cargo test -p amigo_net --features rollback_net
    cargo test -p amigo_audio --features audio_graph

# Doc tests for the crates CI covers
test-doc:
    cargo test --doc -p amigo_tidal_parser -p amigo_audio_pipeline

# Run everything CI runs (catch failures before pushing)
ci: fmt-check check clippy clippy-features test test-features test-doc doc

# Build API docs for the same crates as CI, failing on doc warnings
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p amigo_core -p amigo_render -p amigo_audio -p amigo_input -p amigo_tidal_parser -p amigo_audio_pipeline -p amigo_engine
