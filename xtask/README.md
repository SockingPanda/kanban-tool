# xtask

`xtask` 是 Rust 编写的离线仓库工具。它负责仓库语义校验与生成、依赖图、affected 规划、benchmark、
package 和 provenance 证据，读取 workspace metadata、protocol catalog 和提交的 artifact；它不拥有
产品运行时、canonical database 或第二条 mutation path。

常规开发通过根 `justfile` 调用它，`justfile` 是稳定入口。所有写入 Cargo target 的命令经
`scripts/cargo-build-lock.sh` 共用一把构建锁。默认目录为
`/media/zebra/T7_Linux_Work/projects/Personal/labs/.cache/kanban-tool/cargo-target`；其他主机或隔离工具测试可用
`KANBAN_CARGO_TARGET_ROOT` 指定同一主机的共享目录。显式 `CARGO_TARGET_DIR` 必须与该目录一致。
锁内的 Cargo 构建、nextest 和 libtest 默认均使用 8 个并发执行单元；不同命令仍串行持有共享锁。
可用 `KANBAN_CARGO_BUILD_JOBS`、`KANBAN_TEST_THREADS` 调整本机默认值，工具自身的显式环境变量
优先；设为 `auto` 则不注入对应的工具环境变量，采用工具或仓库配置。

当前命令组如下：

- `affected`：根据基线和工作树变更规划、输出并执行受影响的仓库 gate。
- `docs check`：验证文档链接、`include_str!` 目标、crate map 和 ADR index。
- `schema generate|check|audit`：生成并校验 protocol/schema artifact，以及 contract/surface inventory。
- `deps check`：根据 Cargo workspace metadata 校验依赖 graph 和 owner 边界。
- `agents check`：校验仓库契约、技能包结构、根路由与本地技能的一致性，以及根契约、技能正文和
  文档治理/协作指南中的显式技能引用；同时校验 active recipe/package map。不读取全局技能目录。
- `tooling check`：校验 active repository tooling 不含 `.py`、`python`/`python3` 或 Shell 内嵌 Python 入口。
- `web-assets check --root PATH [--dir apps/web/dist]`：使用共享 `kanban-web-artifact` verifier
  校验当前 Web dist；默认读取 `apps/web/dist`，输出 build ID、payload 数量和总 bytes。
  `just web-artifact-check` 只执行此检查，`just web-check` 按 `web-build → web-artifact-check`
  顺序先生成 fresh dist 再校验。
- `package cli`：先验证与 workspace version 一致的 `apps/web/dist` Web artifact，再从 immutable
  snapshot 将 manifest 和全部 payload exact bytes 携带到 `/usr/share/kanban-tool/web`，最后构建
  standalone `kanban` Debian package。
- `release check|receipt`：校验 Stage09 release-proof ledger、09A real-host browser evidence、
  当前 HEAD/base 与 generated contract provenance，并以 no-follow 原子写入 deterministic receipt。
  `receipt` 只允许将 `kanban serve`/临时 canonical DB 的 live health、runtime、manifest、PID 和
  migration evidence 写入结果；axe/visual/performance/stress/package 在 09A 明确保持 `pending`，
  不得被 mock/preview lane 冒充完成。根 `just release-proof-09a` 在 host PID 仍存活时闭合
  `check → serve/restart → browser → receipt` DAG；不要在 host 退出后单独重放 receipt。
- `release package`：消费 `just desktop-package-proof` 生成的 Desktop package evidence；在
  Desktop/sidecar 仍存活时直接读取 `/proc` identity、argv、`/proc/exe` path/SHA 与固定 8721
  listener owner，重新验证 Deb、bundled Web manifest/payload exact bytes、live runtime/manifest，
  并验证隔离 canonical DB 的 rollback path 保持 dev:inode、content SHA 与 seed 不变（typed
  Turso fingerprint 前后均须存在，但重启 readback host 可能刷新 file metadata），且必须有
  Desktop artifact/configuration validation marker。clean worktree 的 formal 模式通过后以
  no-follow 原子写入独立 `desktop-package-receipt.json`；dirty targeted proof 使用
  `release package --diagnostic` 写入独立 `desktop-package-diagnostic-evidence.json` 并执行同一
  语义校验，但绝不写 formal receipt。它不修改 09A ledger，也不把 Web browser gate 冒充
  package gate。

仓库工具的 ownership 是：Rust/`xtask` 持有语义校验、生成、依赖图、affected、benchmark、package 和
provenance；Shell 只负责编排平台工具、环境与进程；frontend TypeScript 与外部平台命令按各自 owner
维护。新增仓库不变量优先放入 Rust 类型、测试或 `xtask`。

## 仓库与文档检查

`just repo-check` 组合 diff、agents、依赖边界、tooling 和文档结构检查；这些检查仍需要编译 xtask。
`just docs-structure-check` 只调用现有 `docs check`，适合普通 Markdown 与导航改动。
`just docs-check` 保留 workspace rustdoc、doctest，并调用同一个结构检查入口；被 Rust include 的
文档、Rust 示例和公开 Rust 文档契约变化仍需要这个完整 gate。根契约、技能和治理指南还需
`just agents-check`。具体入口由根 `justfile` 持有。

affected 调用时显式指定本任务的比较基线，例如 `just affected-plan base=<起点提交>`；它合并基线以来
已提交、暂存、工作树和未跟踪路径，不把默认 `main` 当成所有任务的父分支。普通 Markdown 选择结构
检查；根契约、技能和治理指南补 agents 检查；Rust `include_str!` 引用的文档选择完整 docs gate。
动态 include 的已知前缀限定保守检查范围，无法确定来源或已删除的文档升级为完整检查。
重命名前后的路径均参与判定。文档与代码混合时合并各自 gate，完整 docs gate 覆盖结构检查后去重。
JSON 保留 `base`、`changed_files`、`classifications`、`recipes`、`sources`，recipe 名称与 just 入口一致。

## CLI package

通过 `just cli-package`（或 `xtask package cli --format deb`）构建 standalone `kanban` Debian package。
package 命令只接受 `scripts/cargo-build-lock.sh` 传入的 inherited Cargo build lock proof 和共享的
`CARGO_TARGET_DIR`，并拒绝 source tree 内、含 symlink/non-directory component 或被覆盖的 target。
workspace fingerprint/build/deps 的失效范围由 `cargo metadata` 的当前 workspace package 列表精确限定；
dep-info 会解析 makefile continuation/escape 后确认依赖来自当前 canonical `crates/`。

package 在构建前验证与 workspace version 相符的 `apps/web/dist`，然后只从冻结 snapshot 写入
`/usr/share/kanban-tool/web`；layout gate 会解包并用共享 verifier 核对 inventory、hash、build ID 和
bytes。target root、release tree、private temp/staging 目录和最终 `.deb` 发布都以 no-follow
regular/directory 检查、single-linked 文件、private prefix/parent identity 及同 filesystem rename
约束；已有 single-linked regular package 只会在成功构建后通过原子 rename 替换，symlink/hardlink/non-regular
destination 和失败路径都不会被覆盖。
这些校验面向 dedicated/cooperative target owner，可防止普通协作式误写和漂移，但不承诺抵抗 hostile
same-UID、`CAP_DAC_OVERRIDE` 或同 inode ABA 攻击。package 不设置 `CARGO_HOME`，沿用 Cargo 默认 home。
