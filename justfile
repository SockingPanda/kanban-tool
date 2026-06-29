set positional-arguments
set shell := ["bash", "-cu"]

audit-ignore-flags := "--ignore RUSTSEC-2024-0370 --ignore RUSTSEC-2024-0411 --ignore RUSTSEC-2024-0412 --ignore RUSTSEC-2024-0413 --ignore RUSTSEC-2024-0414 --ignore RUSTSEC-2024-0415 --ignore RUSTSEC-2024-0416 --ignore RUSTSEC-2024-0417 --ignore RUSTSEC-2024-0418 --ignore RUSTSEC-2024-0419 --ignore RUSTSEC-2024-0420 --ignore RUSTSEC-2024-0429 --ignore RUSTSEC-2024-0436 --ignore RUSTSEC-2025-0075 --ignore RUSTSEC-2025-0080 --ignore RUSTSEC-2025-0081 --ignore RUSTSEC-2025-0098 --ignore RUSTSEC-2025-0100 --ignore RUSTSEC-2026-0186"

fmt:
    cargo fmt -- --check

fmt-check:
    cargo fmt -- --check

fix *args:
    scripts/cargo-build-lock.sh -- cargo clippy --fix --tests --allow-dirty "$@"

clippy:
    just clippy-core

clippy-p package:
    scripts/cargo-build-lock.sh -- cargo clippy -p {{package}} --tests -- -D warnings

check: check-core

check-core:
    scripts/cargo-build-lock.sh -- cargo check --tests \
        -p kanban-core \
        -p kanban-entity \
        -p kanban-indexer \
        -p kanban-search \
        -p kanban-graph \
        -p kanban-vector \
        -p kanban-derived-io \
        -p kanban-helper-protocol \
        -p kanban-labels \
        -p kanban-context \
        -p kanban-sqlite \
        -p kanban-local \
        -p kanban-server \
        -p kanban-cli

check-helpers:
    scripts/cargo-build-lock.sh -- cargo check --tests \
        -p kanban-vector-lancedb \
        -p kanban-graph-oxigraph

check-full:
    just check-core
    just check-helpers

test *args:
    just test-core "$@"

test-p package *args:
    shift; if cargo nextest --version >/dev/null 2>&1; then scripts/cargo-build-lock.sh -- cargo nextest run -p {{package}} --no-fail-fast "$@"; else scripts/cargo-build-lock.sh -- cargo test -p {{package}} "$@"; fi

check-p package:
    scripts/cargo-build-lock.sh -- cargo check -p {{package}} --tests

rust-fast:
    just fmt
    just check-core
    just test-core
    just clippy-core

test-core *args:
    if cargo nextest --version >/dev/null 2>&1; then scripts/cargo-build-lock.sh -- cargo nextest run \
        -p kanban-core \
        -p kanban-entity \
        -p kanban-indexer \
        -p kanban-search \
        -p kanban-graph \
        -p kanban-vector \
        -p kanban-derived-io \
        -p kanban-helper-protocol \
        -p kanban-labels \
        -p kanban-context \
        -p kanban-sqlite \
        -p kanban-local \
        -p kanban-server \
        -p kanban-cli \
        --no-fail-fast "$@"; else scripts/cargo-build-lock.sh -- cargo test \
        -p kanban-core -p kanban-entity -p kanban-indexer -p kanban-search \
        -p kanban-graph -p kanban-vector -p kanban-derived-io \
        -p kanban-helper-protocol -p kanban-labels -p kanban-context \
        -p kanban-sqlite -p kanban-local -p kanban-server -p kanban-cli "$@"; fi

test-helpers *args:
    if cargo nextest --version >/dev/null 2>&1; then scripts/cargo-build-lock.sh -- cargo nextest run \
        -p kanban-vector-lancedb \
        -p kanban-graph-oxigraph \
        --no-fail-fast "$@"; else scripts/cargo-build-lock.sh -- cargo test \
        -p kanban-vector-lancedb -p kanban-graph-oxigraph "$@"; fi

test-full *args:
    if cargo nextest --version >/dev/null 2>&1; then scripts/cargo-build-lock.sh -- cargo nextest run --workspace --exclude kanban-desktop --no-fail-fast "$@"; else scripts/cargo-build-lock.sh -- cargo test --workspace --exclude kanban-desktop "$@"; fi

clippy-core *args:
    scripts/cargo-build-lock.sh -- cargo clippy --all-targets \
        -p kanban-core -p kanban-entity -p kanban-indexer -p kanban-search \
        -p kanban-graph -p kanban-vector -p kanban-derived-io \
        -p kanban-helper-protocol -p kanban-labels -p kanban-context \
        -p kanban-sqlite -p kanban-local -p kanban-server -p kanban-cli "$@" -- -D warnings

clippy-helpers *args:
    scripts/cargo-build-lock.sh -- cargo clippy --all-targets \
        -p kanban-vector-lancedb -p kanban-graph-oxigraph "$@" -- -D warnings

clippy-full *args:
    just clippy-core "$@"
    just clippy-helpers "$@"

rust-full:
    just fmt
    just check-full
    just test-full
    just clippy-full

web-test:
    pnpm --dir apps/desktop test

web-typecheck:
    pnpm --dir apps/desktop typecheck

web-build:
    pnpm --dir apps/desktop build

desktop-check:
    scripts/prepare-desktop-helper-binaries.sh
    scripts/test-desktop-helper-sidecars.sh
    scripts/cargo-build-lock.sh -- cargo check -p kanban-desktop --tests
    pnpm --dir apps/desktop typecheck
    pnpm --dir apps/desktop test

desktop-build:
    pnpm --dir apps/desktop build

desktop-package:
    scripts/prepare-desktop-helper-binaries.sh
    scripts/test-desktop-helper-sidecars.sh
    scripts/cargo-build-lock.sh -- pnpm --dir apps/desktop tauri build

cli-package:
    scripts/package-cli-linux.sh --format deb

cli-package-layout:
    scripts/test-cli-package-layout.sh

desktop-package-config:
    scripts/test-desktop-package-config.sh

desktop-package-layout:
    scripts/test-desktop-package-layout.sh

smoke:
    scripts/smoke-v1-local.sh

audit:
    cargo deny check
    cargo audit -D warnings {{audit-ignore-flags}}

target-tools:
    scripts/test-cargo-target-tools.sh
    scripts/test-helper-cargo-tree.sh

diff-check:
    git diff --check

affected-plan base="main":
    scripts/affected-validation.py --base "{{base}}" --mode plan

affected-json base="main":
    scripts/affected-validation.py --base "{{base}}" --mode json

affected base="main":
    scripts/affected-validation.py --base "{{base}}" --mode run

affected-self-test:
    scripts/affected-validation.py --self-test

feature-p package features:
    if cargo nextest --version >/dev/null 2>&1; then scripts/cargo-build-lock.sh -- cargo nextest run -p {{package}} --features "{{features}}" --no-fail-fast --no-tests pass; else scripts/cargo-build-lock.sh -- cargo test -p {{package}} --features "{{features}}"; fi
    scripts/cargo-build-lock.sh -- cargo clippy -p {{package}} --all-targets --features "{{features}}" -- -D warnings

release:
    just affected-self-test
    just audit
    just rust-full
    just target-tools
    just cli-package
    just cli-package-layout
    just desktop-package-config
    just desktop-package
    just desktop-package-layout
    just smoke
    just diff-check
