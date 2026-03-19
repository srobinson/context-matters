default:
    @just --list

build:
    cargo build --workspace

release:
    cargo build --workspace --release

install: release web-install
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

# Install frontend dependencies
web-install:
    cd crates/cm-web/frontend && npm install

# Start cm-web (backend + frontend dev server)
web: web-install
    overmind start -f Procfile.dev

# Regenerate TypeScript types from cm-core via ts-rs
gen-types:
    cargo test -p cm-core export_bindings_ 2>/dev/null; true

# Regenerate sqlx offline query cache (commit .sqlx/ after running)
sqlx-prepare:
    cargo sqlx prepare --workspace
