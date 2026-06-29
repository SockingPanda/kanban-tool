# Codex Cloud Environment

本页记录 `kanban-tool` 在 Codex Cloud 中使用的前端/后端验证环境。它只用于云端开发、测试、lint 和 PR diff，不是产品部署方案，也不改变本项目 SQLite-only、本地单用户、localhost Web 和本地 dispatcher 的架构边界。

官方边界：

- Codex Cloud 会在托管容器中 checkout GitHub repo，运行 setup script，再让 agent 执行命令和验证。
- Setup script 阶段可以联网安装依赖；agent 阶段默认离线。
- Cached container 恢复时可以运行 maintenance script 刷新 branch 依赖。
- 如果仓库有 `AGENTS.md`，Codex 会用它找到项目约定和验证命令。

## Environment Settings

在 Codex web 的 environment settings 中为这个仓库建立环境：

| Field | Value |
|---|---|
| Image | Default `universal` image |
| Package versions | Pin Node.js 22；Rust 使用仓库 `rust-toolchain.toml` 的 stable toolchain |
| Setup script | `scripts/codex-cloud-setup.sh` |
| Maintenance script | `scripts/codex-cloud-maintenance.sh` |
| Agent internet access | Off by default |
| Setup internet access | On |
| Domain allowlist | Common dependencies preset is enough for normal setup |

推荐环境变量：

```bash
KANBAN_CARGO_TARGET_ROOT=$HOME/.cache/kanban-tool/cargo-target
KANBAN_CARGO_BUILD_JOBS=auto
KANBAN_TEST_THREADS=auto
CODEX_CLOUD_INSTALL_TAURI_DEPS=1
CODEX_CLOUD_INSTALL_NEXTEST=1
CODEX_CLOUD_PREWARM_RUST=1
CODEX_CLOUD_PREWARM_DESKTOP=0
```

`KANBAN_CARGO_TARGET_ROOT` 是 Cloud 专用 target 目录。项目本地验证仍按 `AGENTS.md` 使用 `just` recipes；Cloud 环境需要这个变量是因为本地默认 target root 是开发机路径，容器中不应依赖它。Codex Cloud environment values are not shell-expanded by the UI, so the repo scripts expand literal `$HOME/...`, `${HOME}/...`, and `~/...` values before passing paths to Cargo. `KANBAN_CARGO_BUILD_JOBS=auto` 和 `KANBAN_TEST_THREADS=auto` 让 Cargo、nextest 和 libtest 使用容器默认并发；只有 Cloud runner 资源紧张时才改成具体数字。

## Installed Surface

Setup script 会准备：

- Rust stable、`rustfmt`、`clippy`。
- `just`。
- `cargo-nextest`，供 `just test`、`just test-full` 和 `just rust-fast` 优先使用。
- Node.js / pnpm 10 / `apps/desktop` dependencies。
- Debian/Ubuntu 上的 protobuf compiler / well-known types，以及 Tauri Linux build dependencies，包括 WebKitGTK、GTK、ayatana appindicator、xdo、rsvg、OpenSSL 和 Debian packaging tools。
- Rust crate cache，通过 `cargo fetch --locked` 预热。

Maintenance script 在 cached container 恢复后刷新：

- Rust components。
- protobuf compiler / well-known types，保证 cached container 也能获得新增系统依赖。
- `just` / `cargo-nextest` 是否仍可用。
- pnpm activation 和 `apps/desktop` dependencies。
- Rust dependency cache。

## Recommended Cloud Tasks

Focused validation from the current diff:

```bash
just affected base=main
```

Daily Rust backend / CLI / SQLite / server API validation excludes the
helper-heavy LanceDB and Oxigraph backend crates:

```bash
just rust-fast
```

Equivalent expanded core steps:

```bash
just fmt
just check-core
just test
just clippy
```

Single package:

```bash
just check-p kanban-cli
just test-p kanban-cli
just clippy-p kanban-cli
```

Helper-heavy backend validation:

```bash
just check-helpers
just test-helpers
just clippy-helpers
just test-full
just clippy-full
```

Use `just rust-full` when a branch touches helper backends or release-sensitive
Rust validation boundaries but does not need desktop/package smoke coverage.

Desktop frontend and Tauri check:

```bash
just web-typecheck
just web-test
just web-build
just desktop-check
```

Release-style gate:

```bash
just release
```

`just release` is intentionally heavy. Use it when the branch touches release-sensitive packaging, desktop package behavior, or cross-surface integration.
It includes `just rust-full`, so helper-heavy crates are checked, tested, and
linted before packaging and smoke checks.

## Prompt Template

Use this prompt for Cloud verification jobs:

```text
Run repository verification for this branch using AGENTS.md.
Use just recipes only; do not call raw cargo build/test/check/clippy directly.
Start with `just affected base=main`.
If validation fails, diagnose the failure and fix only issues introduced by this branch.
Report exact commands run and final pass/fail evidence.
```

For backend-only work:

```text
Verify backend/CLI/SQLite behavior for this branch using AGENTS.md.
Run `just rust-fast` for daily core validation, or `just rust-full` if helper-heavy crates are affected. `just test` and `just clippy` are core defaults; use `just test-full` and `just clippy-full` when helper-heavy crates need explicit coverage.
Fix only branch-caused failures and report command evidence.
```

For desktop work:

```text
Verify desktop frontend and Tauri checks for this branch using AGENTS.md.
Run `just desktop-check`.
If it fails, isolate whether the failure is TypeScript/Vitest/Rust/Tauri system dependency related.
Fix only branch-caused failures and report command evidence.
```

## Boundaries

Codex Cloud is useful for saving local CPU, RAM, and compile time, but keep these boundaries explicit:

- It does not see the developer machine's local SQLite database, localhost services, tmux sessions, or OMX runtime.
- It is not a deployment target for `kanban serve`, dispatcher, or Desktop.
- It must not introduce cloud sync, remote worker, multi-user, RBAC, organization, invite, or SaaS assumptions.
- Tauri packaging can run in Cloud only when Linux system dependencies are present; actual desktop tray/window behavior still needs local manual verification when relevant.
- Agent internet access should stay off unless the task explicitly needs current external information.

## Troubleshooting

If `just` commands try to write to a missing local path, confirm `KANBAN_CARGO_TARGET_ROOT` is set in the environment settings or sourced from `~/.bashrc`.

If `just check` fails in `lance-encoding` with `google/protobuf/empty.proto: File not found`, rerun maintenance or reset the environment cache so `protobuf-compiler` and `libprotobuf-dev` are installed.

If `desktop-package` fails with missing `webkit2gtk`, reset the environment cache and rerun setup with `CODEX_CLOUD_INSTALL_TAURI_DEPS=1`.

If `pnpm --dir apps/desktop install --frozen-lockfile` fails because of the pnpm version, keep Node pinned to 22 and rerun setup; the script activates pnpm 10, which supports this lockfile format.

If `cargo-nextest` install is too slow for a temporary environment, set `CODEX_CLOUD_INSTALL_NEXTEST=0`; `just test` and `just test-full` will fall back to `cargo test`.

## References

- Codex Cloud environments: <https://developers.openai.com/codex/cloud/environments>
- Codex Cloud internet access: <https://developers.openai.com/codex/cloud/internet-access>
- Codex web setup: <https://developers.openai.com/codex/cloud>
