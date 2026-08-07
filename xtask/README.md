# xtask

`xtask` 是 Rust 编写的离线仓库工具。它负责仓库语义校验与生成、依赖图、affected 规划、benchmark、
package 和 provenance 证据，读取 workspace metadata、protocol catalog 和提交的 artifact；它不拥有
产品运行时、canonical database 或第二条 mutation path。

常规开发通过根 `justfile` 调用它，`justfile` 是稳定入口。当前命令组如下：

- `affected`：根据基线和工作树变更规划、输出并执行受影响的仓库 gate。
- `docs check`：验证文档链接、`include_str!` 目标、crate map 和 ADR index。
- `schema generate|check|audit`：生成并校验 protocol/schema artifact，以及 contract/surface inventory。
- `deps check`：根据 Cargo workspace metadata 校验依赖 graph 和 owner 边界。
- `agents check`：校验仓库契约、技能包结构和 active recipe/package map。
- `tooling check`：校验 active repository tooling 不含 `.py`、`python`/`python3` 或 Shell 内嵌 Python 入口。
- `package cli`：构建 standalone `kanban` Debian package。

仓库工具的 ownership 是：Rust/`xtask` 持有语义校验、生成、依赖图、affected、benchmark、package 和
provenance；Shell 只负责编排平台工具、环境与进程；frontend TypeScript 与外部平台命令按各自 owner
维护。新增仓库不变量优先放入 Rust 类型、测试或 `xtask`。

## CLI package

通过 `just cli-package`（或 `xtask package cli --format deb`）构建 standalone `kanban` Debian package。
package 命令只接受 `scripts/cargo-build-lock.sh` 传入的 inherited Cargo build lock proof 和共享的
`CARGO_TARGET_DIR`，并拒绝 source tree 内、含 symlink/non-directory component 或被覆盖的 target。
workspace fingerprint/build/deps 的失效范围由 `cargo metadata` 的当前 workspace package 列表精确限定；
dep-info 会解析 makefile continuation/escape 后确认依赖来自当前 canonical `crates/`。

target root、release tree、private temp/staging 目录和最终 `.deb` 发布都以 no-follow regular/directory
检查、single-linked 文件、private prefix/parent identity 及同 filesystem rename 约束；已有 single-linked
regular package 只会在成功构建后通过原子 rename 替换，symlink/hardlink/non-regular destination 和失败
路径都不会被覆盖。
这些校验面向 dedicated/cooperative target owner，可防止普通协作式误写和漂移，但不承诺抵抗 hostile
same-UID、`CAP_DAC_OVERRIDE` 或同 inode ABA 攻击。package 不设置 `CARGO_HOME`，沿用 Cargo 默认 home。
