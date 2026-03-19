default:
    @just --list

build:
    cargo build --workspace

release:
    cargo build --workspace --release

install: release
    cargo install --path crates/cm-cli

test:
    cargo nextest run --workspace

# Run doctests (nextest doesn't support doctests)
test-doc:
    cargo test --workspace --doc

fmt:
    cargo fmt --all

clippy:
    cargo clippy --workspace --all-targets --fix --allow-dirty -- -D warnings

check: fmt clippy

check-pedantic:
    cargo clippy -p cm-core --all-targets -- -W clippy::pedantic -D warnings

serve-dev:
    cargo run -p cm-cli -- serve

# Run with verbose logging
serve-debug:
    CM_LOG_LEVEL=debug cargo run -p cm-cli -- serve --verbose

# Regenerate sqlx offline query cache (commit .sqlx/ after running)
sqlx-prepare:
    cargo sqlx prepare --workspace
