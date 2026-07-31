# Codex Cloud 环境

本页记录 `kanban-tool` 在 Codex Cloud 中使用的前端 / 后端验证环境。它只用于云端开发、测试、lint 和 PR 差异检查，不是产品部署方案，也不改变本项目仅使用 SQLite、本地单用户、localhost Web 的架构边界。dispatcher 只是仓库内暂时保留的实验性能力，不属于公开支持或云端部署路径。

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

日常 Rust 后端、CLI、SQLite 和服务器 API 验证不包含依赖较重的 LanceDB 与 Oxigraph 后端 crate：

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

依赖较重的辅助后端验证：

```bash
just check-helpers
just test-helpers
just clippy-helpers
just test-full
just clippy-full
```

如果分支修改了辅助后端或影响发布的 Rust 验证边界，但不需要桌面端 / 打包冒烟覆盖，请使用
`just rust-full`。

桌面前端与 Tauri 检查：

```bash
just web-typecheck
just web-test
just web-build
just desktop-check
```

发布级门禁：

```bash
just release
```

`just release` 有意设置得较重。分支影响发布打包、桌面包行为或跨入口集成时才使用它。
它包含 `just audit` 和 `just rust-full`，所以会在打包和冒烟检查前完成依赖公告检查、辅助
crate 验证、测试与 lint。发布入口还要求当前目录是真实仓库根、symbolic `main`、工作树
（含 untracked）干净且 HEAD 与远端 `origin/main` 完全一致；保存/实时的
`origin/derived-projection-v2` tip、三个 source slice ancestry 与 no-merge-base 证明也必须
与 canonical source map 一致。整个 cohort 持有同一把 reentrant build lock，把 deterministic
`KANBAN_BUILD_ID`、source manifest 和语义移植 source map 绑定到 CLI、desktop 与 helper。
发布前会对私有同盘 staged generation 重新检查 source、helper runtime identity、package
payload 与全部 artifact hash。发布机必须是支持 Linux read lease
（`F_SETLEASE`）以及 `renameat2(RENAME_NOREPLACE|RENAME_EXCHANGE)` 的同盘 filesystem；
不支持、已有 writer、任一 xattr/ACL/file capability 或 identity drift 都 fail closed。
safe-path 只用 `O_NOFOLLOW` dirfd `*at` 操作；semantic verifier 通过继承的 pinned tree fd
读取同一 snapshot，并在 kernel lease 内执行前后 semantic/digest verify、parent fsync 和
必要的 atomic rollback。generation 只有在同级 `<generation>.published` marker 以
no-replace durable publish 且 reader 重新验证 marker/tree 后才有 authority；无 marker
generation 不会被采用。rename 前的 `<generation>.publishing` intent 绑定 deterministic
source stage、destination、tree inode 与 digest；只有 exact intent 能在真实 wrapper 重跑时
恢复，unknown destination 原地不动。单文件 replacement 用 exchange 保留 rollback copy，
commit 前 rewalk public path，rollback 前核对双端 inode；drift 时写 retention marker。
临时 tree 只按预先 pinned identity 清理，无法证明安全时保留并告警。成功进程持有
leases/fds 直到 flush 后退出；hermetic test 另用 whole-wrapper hash、default-deny `PATH`
与 exact ordered JSONL trace 锁定命令图。lease 释放后仍须由生产 filesystem 权限保护
cohort root，`0555/0444` 不是永久 hostile-root immutability。
`just release` 只生成并验证可追溯的 release cohort，不执行部署、不授予生产恢复权限，
也不替代 `docs/release/DERIVED_PROJECTION_V2_RECOVERY.md`；实际生产恢复必须严格遵循该
runbook。

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
按照 AGENTS.md 验证当前分支的后端、CLI 和 SQLite 行为。
日常核心验证运行 `just rust-fast`；如果影响依赖较重的辅助 crate，则运行 `just rust-full`。`just test` 和 `just clippy` 默认只覆盖核心范围；辅助 crate 需要显式覆盖时，使用 `just test-full` 和 `just clippy-full`。
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

- 它看不到开发机上的本地 SQLite 数据库、localhost 服务或其他仅存在于开发机的进程与会话。
- 它不是 `kanban serve`、实验性 dispatcher 或 Desktop 的部署目标。
- 它不得引入云同步、远程 worker、多用户、RBAC、组织、邀请或 SaaS 假设。
- 只有 Linux 系统依赖齐全时，才能在 Cloud 中打包 Tauri；实际桌面托盘 / 窗口行为在相关改动中仍需本地人工验证。
- 除非任务明确需要当前外部信息，否则 agent 联网权限应保持关闭。

## 故障排查

如果 `just` 命令试图写入不存在的本地路径，请确认环境设置或 `~/.bashrc` 中的
`KANBAN_CARGO_TARGET_ROOT` 与 `CARGO_TARGET_DIR` 被设为完全相同的路径。

如果 `just check` 在 `lance-encoding` 中因 `google/protobuf/empty.proto: File not found`
失败，请重新运行维护脚本或重置环境缓存，确保安装 `protobuf-compiler` 和 `libprotobuf-dev`。

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
