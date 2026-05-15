default:
    @just --list

CM_LOCAL_BIN := env_var_or_default("CM_LOCAL_BIN", "/Users/alphab/.cargo/bin/cm")
FRONTEND_DIR := "crates/cm-web/frontend"

build:
    cargo build --workspace

clean *ARGS:
    @case "{{ARGS}}" in \
        ""|"--hard") ;; \
        *) echo "Usage: just clean [--hard]" >&2; exit 2 ;; \
    esac
    cargo clean
    cd {{FRONTEND_DIR}} && pnpm run clean
    @if [ "{{ARGS}}" = "--hard" ]; then rm -rf {{FRONTEND_DIR}}/node_modules; fi

build-local:
    CONTEXT_MATTERS_GIT_SHA="$(git rev-parse --short=7 HEAD)" cargo build --release -p cm-cli

release:
    cargo build --workspace --release

install: release web-install
    cargo install --path crates/cm-cli --force

install-local: build-local
    @set -eu; \
    src="$(pwd)/target/release/cm"; \
    dest="{{CM_LOCAL_BIN}}"; \
    case "$dest" in /*) ;; *) dest="$(pwd)/$dest";; esac; \
    if [ "$src" = "$dest" ]; then \
        echo "Built $src"; \
    else \
        mkdir -p "$(dirname "$dest")"; \
        install -m 755 "$src" "$dest"; \
        echo "Installed $dest"; \
    fi

test *ARGS:
    cargo nextest run --workspace {{ARGS}}

# Run doctests (nextest doesn't support doctests)
test-doc:
    cargo test --workspace --doc

# Run criterion benchmarks for cm-capabilities hot paths (ALP-1762)
bench:
    cargo bench -p cm-capabilities

fmt:
    cargo fmt --all

clippy:
    cargo clippy --workspace --fix --allow-dirty -- -D warnings

check: fmt clippy web-check

check-pedantic:
    cargo clippy -p cm-core --all-targets -- -W clippy::pedantic -D warnings

serve-dev:
    cargo run -p cm-cli -- serve

# Run with verbose logging
serve-debug:
    CM_LOG_LEVEL=debug cargo run -p cm-cli -- serve --verbose

# Lint + format check frontend (biome + tsc)
web-check:
    cd {{FRONTEND_DIR}} && pnpm run check && pnpm run typecheck

# Install frontend dependencies
web-install:
    cd {{FRONTEND_DIR}} && pnpm install

# Start cm-web (backend + frontend dev server)
web: web-install
    overmind start -f Procfile.dev

# Regenerate TypeScript types from cm-core + cm-capabilities via ts-rs
gen-types:
    cargo test -p cm-core export_bindings_ 2>/dev/null; true
    cargo test -p cm-capabilities export_bindings_ 2>/dev/null; true

# Regenerate sqlx offline query cache (commit .sqlx/ after running)
sqlx-prepare:
    cargo sqlx prepare --workspace
