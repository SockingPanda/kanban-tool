# xtask

`xtask` 保存离线的 schema、依赖、文档结构检查和 CLI release package 工具。它读取 workspace metadata、
protocol catalog 和提交的 artifact，不拥有产品运行时、canonical database 或第二条 mutation path。

常规开发通过根 `justfile` 调用它。`docs check` 只验证文档链接、`include_str!` 目标、crate map 和
ADR index；`schema check` 才在 protocol/schema contract 发生变化时校验机器 artifact。

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
