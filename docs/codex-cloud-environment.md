# Codex Cloud 环境

本页记录 `kanban-tool` 在 Codex Cloud 中使用的前端 / 后端验证环境。它只用于云端开发、测试、lint
和差异检查，不是产品部署方案。产品仍是本地优先、单机单用户；`kanban serve` 是唯一 host 和
canonical Turso owner，CLI、MCP、Desktop 与 dispatcher 只能通过 typed localhost HTTP/SSE 使用它。
Cloud 不能替代 host，也不改变这一边界。

官方边界：

- Codex Cloud 会在托管容器中检出 GitHub 仓库，运行安装脚本，再让 agent 执行命令和验证。
- 安装脚本阶段可以联网安装依赖；agent 阶段默认离线。
- 缓存容器恢复时可以运行维护脚本刷新分支依赖。
- 如果仓库有 `AGENTS.md`，Codex 会用它找到项目约定和验证命令。

## 环境设置

在 Codex Web 的环境设置中为这个仓库建立环境：

| 字段 | 值 |
|---|---|
| 镜像 | 默认 `universal` 镜像 |
| 软件包版本 | 固定 Node.js 22；Rust 使用仓库 `rust-toolchain.toml` 指定的 stable toolchain |
| 安装脚本 | `scripts/codex-cloud-setup.sh` |
| 维护脚本 | `scripts/codex-cloud-maintenance.sh` |
| Agent 联网权限 | 默认关闭 |
| 安装阶段联网权限 | 开启 |
| 域名白名单 | 常规安装使用“常见依赖”预设即可 |

推荐环境变量：

```bash
KANBAN_CARGO_TARGET_ROOT=$HOME/.cache/kanban-tool/cargo-target
CARGO_TARGET_DIR=$HOME/.cache/kanban-tool/cargo-target
KANBAN_CARGO_BUILD_JOBS=auto
KANBAN_TEST_THREADS=auto
CODEX_CLOUD_INSTALL_TAURI_DEPS=1
CODEX_CLOUD_INSTALL_NEXTEST=1
CODEX_CLOUD_PREWARM_RUST=1
CODEX_CLOUD_PREWARM_DESKTOP=0
```

`KANBAN_CARGO_TARGET_ROOT` 是 Cloud 专用构建产物目录；`CARGO_TARGET_DIR` 必须规范化为同一路径。所有工作树使用这一目标目录，不再派生 `worktrees/<hash>` 子目录，并由目标根目录下唯一的 `.build.lock` 串行写入。项目本地验证仍按 `AGENTS.md` 使用 `just` 配方；未显式设置时，构建脚本使用可移植的 `$HOME/.cache/kanban-tool/cargo-target` 默认值。Cloud 环境显式设置这个变量，是为了让容器缓存位置和并发策略可控。Codex Cloud 界面不会对环境变量值做 shell 展开，因此仓库脚本会先展开字面形式的 `$HOME/...`、`${HOME}/...` 和 `~/...`，再把路径传给 Cargo。`KANBAN_CARGO_BUILD_JOBS=auto` 和 `KANBAN_TEST_THREADS=auto` 让 Cargo、nextest 和 libtest 使用容器默认并发；只有 Cloud 运行器资源紧张时才改成具体数字。

## 安装内容

安装脚本会准备：

- Rust stable、`rustfmt`、`clippy`。
- `just`。
- `cargo-nextest`，供 `just test`、`just test-full` 和 `just rust-fast` 优先使用。
- Node.js、pnpm 10 和 `apps/desktop` 依赖。
- Debian / Ubuntu 上的 protobuf 编译器 / well-known types，以及 Tauri Linux 构建依赖，包括 WebKitGTK、GTK、ayatana appindicator、xdo、rsvg、OpenSSL 和 Debian 打包工具。
- Rust crate 缓存，通过 `cargo fetch --locked` 预热。

维护脚本会在缓存容器恢复后刷新：

- Rust 组件。
- protobuf 编译器 / well-known types，保证缓存容器也能获得新增系统依赖。
- `just` / `cargo-nextest` 是否仍可用。
- pnpm 激活状态和 `apps/desktop` 依赖。
- Rust 依赖缓存。

## 推荐的云端任务

根据当前差异执行针对性验证：

```bash
just affected base=main
```

日常 Rust、CLI、protocol、MCP 和服务器 API 验证覆盖当前 active workspace 的核心 crate；当前没有
需要单独验证的外部 backend lane：

```bash
just rust-fast
```

等价的展开版核心步骤：

```bash
just fmt
just check-core
just test
just clippy
```

依赖审计门禁：

```bash
just audit
```

`just audit` 会运行 `cargo deny check` 和 `cargo audit -D warnings`。这些命令不会写入共用
Cargo target 目录，因此不经过 `scripts/cargo-build-lock.sh`，而是直接运行。

单个包：

```bash
just check-p kanban-cli
just test-p kanban-cli
just clippy-p kanban-cli
```

跨 workspace 的 Rust 验证：

```bash
just rust-full
```

`just affected-plan` 或 `just affected-json` 在 workspace manifest、lockfile、toolchain 和
其他验证边界文件变化时会输出 `full_gate_recommended: true` 以及当前的
`full_gate_commands`（`just rust-full`）。这是可审计的人工复核建议，不会自动执行
package/release 流程。

`rust-full` 是当前 `justfile` 中的完整 Rust recipe，展开为 `fmt-full`、`check-full`、
`test-full` 和 `clippy-full`；它仍不包含 Desktop 与 `xtask`。需要单包验证时，使用当前真实
的 `check-p`、`test-p` 和 `clippy-p`；不要为当前 `justfile` 未定义的路径创建额外 recipe。

当前 Desktop 与打包相关的真实 recipes 包括：

```bash
just web-typecheck
just web-test
just web-build
just desktop-check
just cli-package
just desktop-package
just cli-package-layout
just desktop-package-layout
just desktop-package-config
```

`desktop-package`、`cli-package` 和 layout/config 检查是否执行，取决于本次改动的实际影响；
它们不等于部署或发布完成。

跨 workspace Rust recipe 的等价核心步骤（只在需要逐项证据时使用）：

```bash
just fmt-full
just check-full
just test-full
just clippy-full
```

`docs/release/*.md` 顶部均标为“历史归档”，只保留 baseline/provenance 证据；本页不把历史
release 文档或其中的旧 wrapper 当作当前 Cloud recipe、runtime 或完成证明。

## 提示词模板

Cloud 验证任务可使用下面的提示词：

```text
按照 AGENTS.md 验证当前分支。
只使用 just recipes；不要直接调用原始 cargo build/test/check/clippy 命令。
从 `just affected base=main` 开始。
如果验证失败，诊断原因，并且只修复当前分支引入的问题。
报告实际运行的完整命令，以及最终通过或失败的证据。
```

仅涉及后端时：

```text
按照 AGENTS.md 验证当前分支的 Turso host、后端、CLI 和 MCP 行为。
日常核心验证运行 `just rust-fast`；跨 workspace 时运行 `just rust-full`。`just test` 和
`just clippy` 默认覆盖核心范围；需要显式扩大范围时，使用 `just test-full` 和
`just clippy-full`。
只修复当前分支导致的失败，并报告命令证据。
```

涉及桌面端时：

```text
按照 AGENTS.md 验证当前分支的桌面前端和 Tauri 检查。
运行 `just desktop-check`。
如果失败，判断问题来自 TypeScript、Vitest、Rust 还是 Tauri 系统依赖。
只修复当前分支导致的失败，并报告命令证据。
```

## 边界

Codex Cloud 可以节省本机 CPU、内存和编译时间，但必须明确以下边界：

- 它看不到开发机上的本地 Turso 数据库、localhost 服务或其他仅存在于开发机的进程与会话。
- 它不是 `kanban serve`、同进程 dispatcher 或 Desktop 的部署目标。
- 它不得引入云同步、远程 worker、多用户、RBAC、组织、邀请或 SaaS 假设。
- 只有 Linux 系统依赖齐全时，才能在 Cloud 中打包 Tauri；实际桌面托盘 / 窗口行为在相关改动中仍需本地人工验证。
- 除非任务明确需要当前外部信息，否则 agent 联网权限应保持关闭。

## 故障排查

如果 `just` 命令试图写入不存在的本地路径，请确认环境设置或 `~/.bashrc` 中的
`KANBAN_CARGO_TARGET_ROOT` 与 `CARGO_TARGET_DIR` 被设为完全相同的路径。

如果 `desktop-package` 因缺少 `webkit2gtk` 失败，请重置环境缓存，并以
`CODEX_CLOUD_INSTALL_TAURI_DEPS=1` 重新运行安装脚本。

如果 `pnpm --dir apps/desktop install --frozen-lockfile` 因 pnpm 版本失败，请继续固定
Node 22 并重新运行安装脚本；脚本会激活支持当前锁文件格式的 pnpm 10。

如果临时环境安装 `cargo-nextest` 太慢，可设置 `CODEX_CLOUD_INSTALL_NEXTEST=0`；
`just test` 和 `just test-full` 会回退到 `cargo test`。

## 参考资料

- Codex Cloud 环境：<https://developers.openai.com/codex/cloud/environments>
- Codex Cloud 联网权限：<https://developers.openai.com/codex/cloud/internet-access>
- Codex Web 配置：<https://developers.openai.com/codex/cloud>
