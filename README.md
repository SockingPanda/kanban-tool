# Kanban Tool

**一个放在自己电脑上的看板，也是一条可以被人、脚本和 AI agent 共同使用的可靠工作队列。**

很多看板只负责展示“任务现在在哪一列”。一旦真正开始执行，问题就会变得更具体：

- 这项工作真的可以开始了吗，还是仍被依赖阻塞？
- 是谁接走了任务？执行进程中断后，任务会不会永远卡在“进行中”？
- 人在界面上操作、脚本修改状态、agent 更新任务时，三者看到的是不是同一份事实？
- 几天以后，还能不能说清楚任务为什么被阻塞、何时恢复、经历过哪些尝试？

Kanban Tool 想解决的就是这些问题。

它把每张卡片当成一个有生命周期的工作单元：任务、依赖、评论、执行记录和状态变化都保存在本地 SQLite 中。

桌面界面适合人查看和操作，CLI 适合脚本与 agent。所有受支持入口共享同一套状态机，不会各自维护一份互相打架的状态。

## 你可以拿它做什么

- **管理个人项目**：用看板整理任务，用明确的状态区分“还没想清楚”“暂时做不了”和“现在可以执行”。
- **给本地 agent 一份长期工作清单**：任务不会随着一次对话或一个进程结束而消失。
- **连接人工与自动化流程**：人可以创建、澄清和验收任务，脚本或 worker 可以领取、心跳、提交结果。
- **保留可追溯的过程**：关键变化写入事件记录，每次执行尝试、评论和阻塞原因都能回看。
- **在本机统一多个项目**：一个 SQLite 数据库可以容纳多个 board，同时保持清晰的项目边界。

如果你只想要一个多人在线协作的云看板，Trello、Linear 或 GitHub Projects 会更合适。Kanban Tool 面向的是另一种场景：**单用户、本地优先、需要可靠状态和自动化入口的工作流。**

## 它怎样工作

一项任务通常会经过这样的过程：

```text
主流程：triage → todo / scheduled → ready → running → review → done
异常：  活动状态 → blocked → 重新检查 → triage / todo / scheduled / ready
```

- `triage`：想法还不够清楚，暂时不能执行。
- `todo`：任务已经定义，但依赖尚未完成，或还没有进入执行队列。
- `scheduled`：任务有明确的未来开始时间。
- `ready`：条件已经满足，可以被操作者或显式调用的客户端领取。
- `running`：任务已被领取，拥有一次真实的执行记录。
- `blocked`：执行遇到外部依赖、失败或需要人工输入。
- `review`：工作已经提交，等待人工确认。
- `done`：任务完成。

这里的状态不只是看板上的列。比如 `ready → running` 会以原子事务完成领取、创建 run 并写入事件；超时的领取可以被回收。多个活动状态都可能进入 `blocked`；解除阻塞时，系统会重新检查任务规格、排期和依赖，再决定它应该回到 `triage`、`todo`、`scheduled` 还是 `ready`。

这也是 Kanban Tool 与普通任务列表最核心的区别：**它不只记录你想做什么，也认真记录一项工作是否真的能够执行，以及执行过程中发生了什么。**

## 快速开始

目前项目仍处在“从源码构建、先在真实工作中使用和打磨”的阶段，GitHub 暂无可直接下载的预编译 Release。安装 CLI 需要 [Rust](https://www.rust-lang.org/tools/install)：

```bash
git clone https://github.com/SockingPanda/kanban-tool.git
cd kanban-tool
cargo install --path crates/kanban-cli --bin kanban
```

初始化数据库，创建并选中一个 board：

```bash
kanban init
kanban board create personal --name "Personal"
kanban board use personal
```

创建第一项工作：

```bash
kanban task create "整理项目首页" \
  --description "让第一次访问的人看懂项目" \
  --priority 1
kanban task list
```

你会看到类似下面的摘要：

```text
personal#1 [todo] P1 整理项目首页 · plan: unplanned · steps: 0/0
```

Kanban Tool 要求可执行任务明确说明 execution plan（执行计划）。这个任务只有一步，可以直接标记为不需要拆分；系统会重新计算条件并把它推进到 `ready`：

```bash
kanban task step not-required personal#1 --reason "单步任务"
kanban task list
# personal#1 [ready] P1 整理项目首页 · plan: not_required · steps: 0/0
```

开始任务时，Kanban Tool 会返回一个 claim token（领取凭证）：

```bash
kanban task start personal#1
# Claimed ... token=claim_...
```

完成任务需要带回这个 token，从而避免另一个过期或并发执行者误改状态：

```bash
kanban task done personal#1 --claim-token claim_...
```

常用的下一步：

```bash
kanban task show personal#1 --details
kanban task list --status ready --status running
kanban search "项目首页"
kanban doctor
kanban maintenance status
kanban maintenance run --once
```

## 三种使用方式

### 桌面看板

`apps/desktop` 提供 Tauri 桌面操作界面，适合浏览 board、查看任务详情和进行人工操作。它不是独立的数据层；界面上的状态变化仍然经过与 CLI 相同的 Rust service 和状态机。

当前公开的快速开始只覆盖 CLI；Desktop 尚无预编译下载，需要按照仓库开发流程从源码构建。

### CLI 与本地脚本

`kanban` CLI 覆盖 board、task、dependency、step、comment、event、run、search、backup 和 maintenance 等入口。大多数命令支持 `--json`，因此既适合人直接使用，也适合作为自动化程序的稳定接口。

### 本地 HTTP API

`kanban serve` 默认在 `127.0.0.1:8721` 启动本地 HTTP API 和 SSE 事件流，供 Desktop 或本地脚本使用。它不会提供浏览器版看板，也不会监听公网地址。

## 数据放在哪里

Kanban Tool 不要求远程数据库。在 Linux 上，默认数据通常保存在 XDG 目录：

```text
~/.local/share/kb/kb.db
~/.local/share/kb/attachments/
~/.local/state/kb/logs/
~/.config/kanban/config.toml
```

其他平台或自定义环境可以通过 `kanban config show` 查看实际解析出的路径。

也可以把数据库放进某个项目的 `.kb/` 目录：

```bash
kanban init --db .kb/kb.db
```

SQLite 始终是最终事实来源；搜索、图和向量能力都可以从它重新构建。项目不支持把同一个 SQLite 文件放在 NFS、Dropbox、iCloud Drive 等同步目录中由多台机器共同写入。

## 当前边界

Kanban Tool 有意保持本地、单用户：

- 不提供多用户、团队、邀请或 RBAC。
- 不提供多租户 SaaS。
- 不提供云同步或远程 worker。
- 不支持 PostgreSQL、MySQL 或 MongoDB 后端。
- localhost API 只服务本机界面和本地脚本，不是公网协作 API。
- 仓库中保留的实验性 dispatch 命令不属于公开支持能力；自动化请使用 CLI 或本机 API 显式编排。

这些不是“以后再补的企业功能清单”，而是当前产品边界。它让项目可以把精力放在本地任务状态、恢复能力、审计记录和人机协作上。

## 想深入了解

不需要从头读完整个文档包。可以按问题选择入口：

| 你想了解什么 | 从这里开始 |
|---|---|
| 产品范围和核心概念 | [`docs/SPEC.md`](docs/SPEC.md) |
| CLI 命令和输出 | [`docs/CLI_SPEC.md`](docs/CLI_SPEC.md) |
| 状态为什么这样流转 | [`docs/STATE_MACHINE.md`](docs/STATE_MACHINE.md) |
| 本地 API 与 SSE | [`docs/API_SPEC.md`](docs/API_SPEC.md) |
| Rust crate、进程和数据流 | [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) |
| 数据对象、ID 与事件 | [`docs/DATA_MODEL.md`](docs/DATA_MODEL.md) |
| JSON schema 与公开契约 | [`docs/SCHEMA_CONTRACTS.md`](docs/SCHEMA_CONTRACTS.md) |

[`KANBAN_SPEC_BUNDLE.md`](KANBAN_SPEC_BUNDLE.md) 是主要文档的单文件同步快照，适合离线阅读或一次性交给其他工具处理。

## 参与开发

项目是一个 Rust workspace，桌面端位于 `apps/desktop`。开始修改前请先阅读 [`AGENTS.md`](AGENTS.md)，其中记录了架构边界和验证约定。

查看可用的开发命令：

```bash
just --summary
```

针对当前改动选择最小验证集：

```bash
just affected-plan base="main"
just affected base="main"
```

README 和其他规范源文档发生变化后，需要同步检查单文件文档包：

```bash
just spec-bundle-generate
just spec-bundle-check
```

Linux CLI 可以构建为独立的 Debian 包：

```bash
./scripts/package-cli-linux.sh --format deb \
  --no-default-features \
  --features tantivy-backend,oxigraph-backend
```

桌面包与 CLI 包彼此独立；桌面包不会自动安装系统级 `kanban` 命令。

当前工程验证和安装打包主要围绕 Debian / Ubuntu；其他平台可以从源码尝试，但项目暂未提供同等完成度的安装包承诺。

## 许可证

Kanban Tool 使用 [Apache License 2.0](LICENSE) 开源。
