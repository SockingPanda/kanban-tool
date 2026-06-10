set positional-arguments
set shell := ["bash", "-cu"]

fmt:
    cargo fmt -- --check

fmt-check:
    cargo fmt -- --check

fix *args:
    cargo clippy --fix --tests --allow-dirty "$@"

clippy:
    cargo clippy --workspace --all-targets --exclude kanban-desktop -- -D warnings

check:
    cargo check --workspace --exclude kanban-desktop --tests

test *args:
    if cargo nextest --version >/dev/null 2>&1; then cargo nextest run --workspace --exclude kanban-desktop --no-fail-fast "$@"; else cargo test --workspace --exclude kanban-desktop "$@"; fi

test-p package *args:
    shift; if cargo nextest --version >/dev/null 2>&1; then cargo nextest run -p {{package}} --no-fail-fast "$@"; else cargo test -p {{package}} "$@"; fi

check-p package:
    cargo check -p {{package}} --tests

rust-fast:
    cargo fmt -- --check
    cargo check --workspace --exclude kanban-desktop --tests
    if cargo nextest --version >/dev/null 2>&1; then cargo nextest run --workspace --exclude kanban-desktop --no-fail-fast; else cargo test --workspace --exclude kanban-desktop; fi
    cargo clippy --workspace --all-targets --exclude kanban-desktop -- -D warnings

web-test:
    pnpm --dir apps/desktop test

web-typecheck:
    pnpm --dir apps/desktop typecheck

web-build:
    pnpm --dir apps/desktop build

desktop-check:
    pnpm --dir apps/desktop typecheck
    pnpm --dir apps/desktop test
    pnpm --dir apps/desktop build
    pnpm --dir apps/desktop tauri build

feature-p package features:
    if cargo nextest --version >/dev/null 2>&1; then cargo nextest run -p {{package}} --features "{{features}}" --no-fail-fast; else cargo test -p {{package}} --features "{{features}}"; fi
    cargo clippy -p {{package}} --all-targets --features "{{features}}" -- -D warnings

release:
    just rust-fast
    just feature-p kanban-sqlite tantivy-backend
    just feature-p kanban-sqlite graph-oxigraph
    just feature-p kanban-sqlite vector-lancedb
    just feature-p kanban-sqlite tantivy-backend,graph-oxigraph,vector-lancedb
    just feature-p kanban-search tantivy-backend
    just feature-p kanban-graph graph-oxigraph
    just feature-p kanban-vector vector-lancedb
    just feature-p kanban-cli tantivy-backend
    just feature-p kanban-cli graph-oxigraph
    just feature-p kanban-cli vector-lancedb
    just feature-p kanban-cli tantivy-backend,graph-oxigraph,vector-lancedb
    just feature-p kanban-server tantivy-backend
    just feature-p kanban-server graph-oxigraph
    just feature-p kanban-server vector-lancedb
    just feature-p kanban-server tantivy-backend,graph-oxigraph,vector-lancedb
    just desktop-check
