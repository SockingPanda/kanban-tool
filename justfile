set positional-arguments
set shell := ["bash", "-cu"]

fmt:
    cargo fmt -- --check

fmt-check:
    cargo fmt -- --check

fix *args:
    scripts/cargo-build-lock.sh -- cargo clippy --fix --tests --allow-dirty "$@"

clippy:
    scripts/cargo-build-lock.sh -- cargo clippy --workspace --all-targets --exclude kanban-desktop -- -D warnings

clippy-p package:
    scripts/cargo-build-lock.sh -- cargo clippy -p {{package}} --tests -- -D warnings

check:
    scripts/cargo-build-lock.sh -- cargo check --workspace --exclude kanban-desktop --tests

test *args:
    if cargo nextest --version >/dev/null 2>&1; then scripts/cargo-build-lock.sh -- cargo nextest run --workspace --exclude kanban-desktop --no-fail-fast "$@"; else scripts/cargo-build-lock.sh -- cargo test --workspace --exclude kanban-desktop "$@"; fi

test-p package *args:
    shift; if cargo nextest --version >/dev/null 2>&1; then scripts/cargo-build-lock.sh -- cargo nextest run -p {{package}} --no-fail-fast "$@"; else scripts/cargo-build-lock.sh -- cargo test -p {{package}} "$@"; fi

check-p package:
    scripts/cargo-build-lock.sh -- cargo check -p {{package}} --tests

rust-fast:
    cargo fmt -- --check
    scripts/cargo-build-lock.sh -- cargo check --workspace --exclude kanban-desktop --tests
    if cargo nextest --version >/dev/null 2>&1; then scripts/cargo-build-lock.sh -- cargo nextest run --workspace --exclude kanban-desktop --no-fail-fast; else scripts/cargo-build-lock.sh -- cargo test --workspace --exclude kanban-desktop; fi
    scripts/cargo-build-lock.sh -- cargo clippy --workspace --all-targets --exclude kanban-desktop -- -D warnings

web-test:
    pnpm --dir apps/desktop test

web-typecheck:
    pnpm --dir apps/desktop typecheck

web-build:
    pnpm --dir apps/desktop build

desktop-check:
    scripts/cargo-build-lock.sh -- cargo check -p kanban-desktop --tests
    pnpm --dir apps/desktop typecheck
    pnpm --dir apps/desktop test

desktop-build:
    pnpm --dir apps/desktop build

desktop-package:
    scripts/cargo-build-lock.sh -- pnpm --dir apps/desktop tauri build

cli-package:
    scripts/package-cli-linux.sh --format deb

smoke:
    scripts/smoke-v1-local.sh

target-tools:
    scripts/test-cargo-target-tools.sh

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
    just affected
    just cli-package
    just desktop-package
    just smoke
    just diff-check
