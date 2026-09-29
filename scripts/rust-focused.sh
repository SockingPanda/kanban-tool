#!/usr/bin/env bash
# 仅编排 Cargo；包范围由 xtask/affected 的 Cargo 依赖图决定。
set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

# 保留既有无参数 rust-fast 行为，避免悄悄改变外部调用者的覆盖范围。
if (($# == 0)); then
    just fmt
    just check-core
    just test-core
    just clippy-core
    exit 0
fi

packages=()
for package in "$@"; do
    # 不接受通配符或额外 Cargo 参数；每项都必须是一个包名。
    if [[ ! "$package" =~ ^[a-zA-Z0-9_][a-zA-Z0-9_-]*$ ]]; then
        printf '非法 Rust 包名：%s\n' "$package" >&2
        exit 2
    fi
    packages+=(-p "$package")
done

# 与 rust-full 一致：普通 Rust gate 不执行 Desktop 资源打包。
export TAURI_CONFIG='{"bundle":{"resources":[]}}'
cargo fmt "${packages[@]}" -- --check
scripts/cargo-build-lock.sh -- cargo check --locked --tests "${packages[@]}"

# 两个 runner 都只运行 test targets；doctest 继续由 docs-check 独立负责。
# 定向开发默认 fail-fast；完整 gate 的 no-fail-fast 行为保持不变。
# 包名已明确给定；仅有编译目标的包允许零测试，与 cargo test 保持一致。
if cargo nextest --version >/dev/null 2>&1; then
    scripts/cargo-build-lock.sh -- cargo nextest run --locked --tests --no-tests pass "${packages[@]}"
else
    scripts/cargo-build-lock.sh -- cargo test --locked --tests "${packages[@]}"
fi
scripts/cargo-build-lock.sh -- cargo clippy --locked --all-targets "${packages[@]}" -- -D warnings
