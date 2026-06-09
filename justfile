set positional-arguments
set shell := ["bash", "-cu"]

fmt:
    cargo fmt -- --check

fmt-check:
    cargo fmt -- --check

fix *args:
    cargo clippy --fix --tests --allow-dirty "$@"

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

test *args:
    if cargo nextest --version >/dev/null 2>&1; then cargo nextest run --workspace --no-fail-fast "$@"; else cargo test --workspace "$@"; fi

test-p package *args:
    if cargo nextest --version >/dev/null 2>&1; then cargo nextest run -p {{package}} --no-fail-fast "$@"; else cargo test -p {{package}} "$@"; fi

check-p package:
    cargo check -p {{package}} --all-targets

release:
    cargo fmt -- --check
    cargo test --workspace
    cargo clippy --workspace --all-targets -- -D warnings
