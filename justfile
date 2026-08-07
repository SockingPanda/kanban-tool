set positional-arguments
set shell := ["bash", "-cu"]

# `cargo audit` 使用 `-D warnings`；以下精确 ID 是当前显式 allowlist 跟踪的
# 传递依赖安全公告基线。
# - RUSTSEC-2024-0370：GTK3/glib 宏依赖引入 `proc-macro-error`。
# - RUSTSEC-2024-0411..0420：Tauri/wry 引入的 gtk-rs GTK3 绑定安全公告。
# - RUSTSEC-2024-0429：GTK3 栈中的 glib 未定义行为公告；即使 `cargo-deny`
#   未在该依赖图中遇到它，`cargo audit` 仍会报告。
# - RUSTSEC-2024-0436：传递宏使用带来的 `paste` 未维护警告。
# - RUSTSEC-2025-0075/0080/0081/0098/0100：`urlpattern`/`tauri-utils` 引入的
#   `rust-unic` crates。
audit-ignore-flags := "--ignore RUSTSEC-2024-0370 --ignore RUSTSEC-2024-0411 --ignore RUSTSEC-2024-0412 --ignore RUSTSEC-2024-0413 --ignore RUSTSEC-2024-0414 --ignore RUSTSEC-2024-0415 --ignore RUSTSEC-2024-0416 --ignore RUSTSEC-2024-0417 --ignore RUSTSEC-2024-0418 --ignore RUSTSEC-2024-0419 --ignore RUSTSEC-2024-0420 --ignore RUSTSEC-2024-0429 --ignore RUSTSEC-2024-0436 --ignore RUSTSEC-2025-0075 --ignore RUSTSEC-2025-0080 --ignore RUSTSEC-2025-0081 --ignore RUSTSEC-2025-0098 --ignore RUSTSEC-2025-0100"

fmt:
    cargo fmt -p kanban-core -p kanban-service -p kanban-protocol -p kanban-client -p kanban-server -p kanban-cli -p kanban-mcp -- --check

fmt-check: fmt

fmt-full:
    cargo fmt -p kanban-core -p kanban-service -p kanban-protocol -p kanban-client -p kanban-server -p kanban-cli -p kanban-mcp -p kanban-desktop -p xtask -- --check

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
        -p kanban-service \
        -p kanban-protocol \
        -p kanban-client \
        -p kanban-server \
        -p kanban-cli \
        -p kanban-mcp

check-full:
    scripts/cargo-build-lock.sh -- cargo check --workspace --tests

test *args:
    just test-core "$@"

test-p package *args:
    shift; if cargo nextest --version >/dev/null 2>&1; then scripts/cargo-build-lock.sh -- cargo nextest run -p {{package}} --no-fail-fast "$@"; else scripts/cargo-build-lock.sh -- cargo test -p {{package}} "$@"; fi

check-p package:
    scripts/cargo-build-lock.sh -- cargo check -p {{package}} --tests

check-windows-p package:
    scripts/cargo-build-lock.sh -- cargo check --locked -p {{package}} --target x86_64-pc-windows-gnu

rust-fast:
    just fmt
    just check-core
    just test-core
    just clippy-core

test-core *args:
    if cargo nextest --version >/dev/null 2>&1; then scripts/cargo-build-lock.sh -- cargo nextest run \
        -p kanban-core \
        -p kanban-service \
        -p kanban-protocol \
        -p kanban-client \
        -p kanban-server \
        -p kanban-cli \
        -p kanban-mcp \
        --no-fail-fast "$@"; else scripts/cargo-build-lock.sh -- cargo test \
        -p kanban-core -p kanban-service -p kanban-protocol \
        -p kanban-client -p kanban-server -p kanban-cli -p kanban-mcp "$@"; fi

test-full *args:
    if cargo nextest --version >/dev/null 2>&1; then scripts/cargo-build-lock.sh -- cargo nextest run --workspace --no-fail-fast "$@"; else scripts/cargo-build-lock.sh -- cargo test --workspace "$@"; fi

clippy-core *args:
    scripts/cargo-build-lock.sh -- cargo clippy --all-targets \
        -p kanban-core -p kanban-service -p kanban-protocol \
        -p kanban-client -p kanban-server -p kanban-cli -p kanban-mcp "$@" -- -D warnings

clippy-full *args:
    scripts/cargo-build-lock.sh -- cargo clippy --workspace --all-targets "$@" -- -D warnings

rust-full:
    just fmt-full
    just check-full
    just test-full
    just clippy-full

deps-check:
    scripts/cargo-build-lock.sh -- cargo run --locked -p xtask --bin xtask -- deps check

agents-check:
    scripts/cargo-build-lock.sh -- cargo run --locked -p xtask --bin xtask -- agents check

docs-check:
    scripts/cargo-build-lock.sh -- cargo doc --workspace --no-deps
    scripts/cargo-build-lock.sh -- cargo test --doc --workspace
    scripts/cargo-build-lock.sh -- cargo run --locked -p xtask --bin xtask -- docs check

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

single-host-dependency-gate:
    python3 -B scripts/check-single-host-dependencies.py

desktop-build:
    pnpm --dir apps/desktop build

desktop-package:
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
    python3 -B scripts/test_release_safe_path.py
    scripts/test-package-cli-linux-safe.sh

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
    if cargo nextest --version >/dev/null 2>&1; then scripts/cargo-build-lock.sh -- cargo nextest run --locked -p {{package}} --features "{{features}}" --no-fail-fast --no-tests pass; else scripts/cargo-build-lock.sh -- cargo test --locked -p {{package}} --features "{{features}}"; fi
    scripts/cargo-build-lock.sh -- cargo clippy --locked -p {{package}} --all-targets --features "{{features}}" -- -D warnings

schema-generate:
    scripts/cargo-build-lock.sh -- cargo run --locked -p xtask --bin xtask -- schema generate

schema-check:
    scripts/cargo-build-lock.sh -- cargo run --locked -p xtask --bin xtask -- schema check

# CI 的完整编排只组合真实 recipes；日常窄 gate 仍保持 core 与单包范围。
ci-full:
    just rust-full
    just desktop-check
    just web-build
    just docs-check
    just schema-contract
    just deps-check
    just agents-check
    just audit
    just smoke
    just diff-check

schema-fmt:
    cargo fmt -p kanban-protocol -p xtask -- --check

schema-tool:
    scripts/cargo-build-lock.sh -- cargo check --locked -p xtask --tests
    if cargo nextest --version >/dev/null 2>&1; then scripts/cargo-build-lock.sh -- cargo nextest run --locked -p xtask --no-fail-fast; else scripts/cargo-build-lock.sh -- cargo test --locked -p xtask; fi
    scripts/cargo-build-lock.sh -- cargo clippy --locked -p xtask --all-targets -- -D warnings

schema-dependency-isolation-self-test:
    python3 -B scripts/test_schema_dependency_isolation.py
    python3 -B scripts/test_schema_recipe_witness.py

schema-dependency-isolation:
    just schema-dependency-isolation-self-test
    python3 -B scripts/schema_dependency_policy.py
    scripts/test-schema-cargo-tree.sh

schema-adoption-witness-self-test:
    python3 -B scripts/test_schema_adoption_witnesses.py

schema-adoption-witness:
    just schema-adoption-witness-self-test
    python3 -B scripts/schema_adoption_witnesses.py --root .

schema-surface-audit:
    if cargo nextest --version >/dev/null 2>&1; then scripts/cargo-build-lock.sh -- cargo nextest run --locked -p kanban-server api_route_catalog_matches_exact_contract_catalog --no-fail-fast; else scripts/cargo-build-lock.sh -- cargo test --locked -p kanban-server api_route_catalog_matches_exact_contract_catalog; fi
    if cargo nextest --version >/dev/null 2>&1; then scripts/cargo-build-lock.sh -- cargo nextest run --locked -p kanban-cli clap_leaf_commands_match_exact_contract_catalog --no-fail-fast; else scripts/cargo-build-lock.sh -- cargo test --locked -p kanban-cli clap_leaf_commands_match_exact_contract_catalog; fi

schema-contract:
    just schema-dependency-isolation
    just schema-fmt
    just feature-p kanban-protocol schema
    just schema-tool
    just schema-check
    just schema-surface-audit
    just schema-adoption-witness

schema-audit-closed:
    just schema-adoption-witness
    scripts/cargo-build-lock.sh -- cargo run --locked -p xtask --bin xtask -- schema audit --require-closed

schema *args:
    scripts/cargo-build-lock.sh -- cargo run --locked -p xtask --bin xtask -- schema "$@"

deps *args:
    scripts/cargo-build-lock.sh -- cargo run --locked -p xtask --bin xtask -- deps "$@"

agents *args:
    scripts/cargo-build-lock.sh -- cargo run --locked -p xtask --bin xtask -- agents "$@"
