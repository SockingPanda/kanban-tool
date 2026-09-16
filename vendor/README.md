# Tantivy 安全补丁

根 `Cargo.toml` 通过 `[patch.crates-io]` 使用 [`tantivy-0.26.1`](tantivy-0.26.1/)。
该目录保留 Tantivy `0.26.1` 的发布源码，只回移上游对 `lru` 的依赖升级。
根 `Cargo.lock` 解析真实发布的 `lru 0.18.2`，修复
[RUSTSEC-2026-0253](https://rustsec.org/advisories/RUSTSEC-2026-0253.html)。

## 来源与变更

| 项目 | 可核对来源 |
| --- | --- |
| 原始发布包 | [tantivy-0.26.1.crate](https://static.crates.io/crates/tantivy/tantivy-0.26.1.crate) |
| 发布包 SHA-256 | `edde6a10743fff00a4e1a8c9ef020bf5f3cbad301b7d2d39f2b07f123c4eac07` |
| 发布包记录的 Git commit | [`d8f4c0b703120ed98f06297724dc1522df6019b9`](https://github.com/quickwit-oss/tantivy/commit/d8f4c0b703120ed98f06297724dc1522df6019b9)；原记录保存在 [`.cargo_vcs_info.json`](tantivy-0.26.1/.cargo_vcs_info.json) |
| 回移补丁 | [上游 PR #3034](https://github.com/quickwit-oss/tantivy/pull/3034)，commit [`5ca39332002c2c87fb5d2abc707cf527b3319d42`](https://github.com/quickwit-oss/tantivy/commit/5ca39332002c2c87fb5d2abc707cf527b3319d42) |
| 许可证 | MIT，保留上游 [`LICENSE`](tantivy-0.26.1/LICENSE) 与 [`AUTHORS`](tantivy-0.26.1/AUTHORS) |

上游补丁把 `lru = "0.16.3"` 改为 `lru = "0.18.2"`。本目录只在发布包的
[`Cargo.toml`](tantivy-0.26.1/Cargo.toml) 和 [`Cargo.toml.orig`](tantivy-0.26.1/Cargo.toml.orig)
同步这一约束；其余文件与原发布包逐字相同。Tantivy 和 Turso 的版本、运行源码与 FTS
feature 保持原值。`lru` 本身来自 crates.io，其版本、来源和校验值由根 `Cargo.lock` 持有。

## 维护边界

`tantivy-0.26.1/` 是第三方源码，明确排除在 workspace members 之外。内含的上游文档、CI、
`Makefile`、Python 生成器和嵌套 `Cargo.lock` 仅随发布包保存；仓库工具不调用它们，实际构建和
安全审计使用根 manifest 与根 lockfile。第一方文档与 tooling 检查跳过这个固定目录，继续检查
本说明和所有第一方入口。`.gitattributes` 为该目录保留原始字节，避免修改上游空白和换行。

更新补丁时重新核对发布包 SHA-256，并将解包目录与本目录比较；允许的差异只有上述两个
manifest 中的 `lru` 约束。不要在第三方运行源码中顺手修复、格式化或翻译内容。

## 退出条件

当与当前 Turso `0.7.2` 兼容的正式 Tantivy 发布版包含 `lru >= 0.18.2` 的依赖约束时，切换到
该 crates.io 发布版，移除根 `[patch.crates-io]`、workspace exclusion、本目录以及对应的固定
vendor 检查边界。退出前重新运行严格依赖审计与 service 的 FTS、搜索和 persistence 回归。
