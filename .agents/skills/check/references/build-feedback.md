# Rust 构建反馈：先辨认工作，再优化配置

仅在处理慢编译、重复构建、门禁编排或构建配置时读取；它不是每次功能任务的额外流程。
命令入口仍是当前 `justfile` 和构建锁，不新增常驻 daemon、任务级 target 或 CI 矩阵框架。
用户只授权静态审核时，下面的诊断与实验只形成方案，不执行。

## 先确定成本属于哪里

| 观察 | 先找证据 | 有证据后再尝试 |
| --- | --- | --- |
| 同一代码、同一命令仍重新编译 | Cargo fingerprint、build.rs 输入、mtime、工具链／环境变化 | 修复失效来源；不先 clean |
| 分包跑时反复重建同一依赖 | 真实 rustc 调用的 features、host/target、profile、编译模式 | 兼容包集中一次 Cargo 调用；保留有意义的配置差异 |
| 修改一个实现就重编很久 | workspace crate codegen、宏、link、增量命中 | 实测 incremental 或开发符号设置 |
| 多个测试二进制链接很慢 | 各 test target 的依赖、大小、链接时间 | 对少量确有重复链接成本的目标做组织调整 |
| 编译并发越高反而越慢 | RSS、swap、I/O 等待和链接器自身线程 | 降低资源竞争；别只看 CPU 核数 |
| 编译几乎没有，运行仍很慢 | nextest 单项时间及初始化／等待次数 | 转交 `$test-design`，不是继续换 linker |

分别记录等待构建锁、编译／链接、测试执行和总 wall time。Cargo timings 是构建证据，
不是测试主体耗时；单个阶段消失不保证整条链按同样秒数缩短。

## 构建配置是缓存命中的条件

比较实际编译时，至少记录 package/version/source、features、host/target、profile、test／普通
编译模式、rustc 版本、RUSTFLAGS、构建脚本输入和相关环境。这里是诊断清单，不是重写 Cargo
内部 fingerprint 算法，也不把这组字段当成稳定的公开缓存协议。

先看触发重建的那次命令。以下是可按当前范围调整的诊断示例，不是新增 recipe；需要执行授权。

```bash
# 已存在的锁入口；保持当前 target，不额外 --target-dir。
CARGO_LOG=cargo::core::compiler::fingerprint=info \
  scripts/cargo-build-lock.sh -- cargo test --locked --tests --no-run \
  -p kanban-service -p kanban-server --timings -vv
```

诊断期间的详细日志可能增加开销，不与关闭日志的正式时间直接比较。随后只在确有需要时使用
`cargo tree -e features -i <实际依赖>` 理解 feature 来源。`cargo tree -d` 找多版本依赖，
不能证明所有同版本多构建，也不能代替实际编译单元证据。

兼容的验证尽量批量执行，不把单个包 A/B/C 的顺序切换变成反复组合 features。feature 差异
可能是必要覆盖：默认／关闭、legacy 开启、平台和工具 crate 的配置不能用 all-features 代替。
聚合构建也不能让某个 leaf 偶然借到另一成员启用的 feature；manifest 与必要的独立编译继续验证。
同一 source/版本在 host、target 或不同编译模式下构建多次，有时是正确行为，不应强行去重。

## 精简门禁，不降低反馈质量

先展开实际 recipe，区分带包名、无参数、完整和文档路径。若 test build 已覆盖 test targets，
后续 Clippy 覆盖其余要求，前置 check 可以不在该链里重复；但 test build、Clippy、非 test cfg、
example/bench、doctest 的覆盖必须逐项对照。没有足够证据时保留窄 check，不机械重复或删除。

保留独立 `check-p` 的快速类型反馈；“这条验收链不需要 check”不等于“开发中不要 check”。
若更快定位静态错误更重要，可比较 Clippy → tests 与 tests → Clippy 的整条时间；失败路径
和首次诊断时延也是成本，不只测一次全绿结束时间。不得让失败后的未运行阶段被报告为通过。

编译-only 包允许没有运行用例，应显式区分“合法零测试”与“本应选中却为空”；runner 的 pass
选项不能成为证明。`--no-run` 只形成构建证据，nextest 过滤只改变执行选择的部分情况也不能
被假定会少编译整个 test binary。

## 依次实验，不批量切换 profile

先锁定当前工具链、依赖和入口。候选配置分别比较，不同时修改 linker、debug、incremental、
opt-level、并发和存储介质。默认不更改 release／性能验收配置，也不重写 lockfile。

| 候选 | 可能收益 | 必须观察的代价与边界 |
| --- | --- | --- |
| workspace incremental | 小实现改动后的重复编译更快 | 额外磁盘与内存、分支切换及不同模式的实际命中；外部 registry 依赖不靠它加速 |
| 开发符号改为 limited 或 line-tables-only | 降低符号、I/O、链接体量 | 调试器类型／变量信息减少；保留所需 backtrace；跨 test/dev 不制造无谓配置抖动 |
| 当前实际 linker 的调整 | 链接是热点时有收益 | 先确认是否已默认 LLD、是否有 override；链接器线程也参与内存竞争 |
| 选定热点依赖提高 opt-level | 重复运行 CPU 密集测试可能变快 | 冷构建/codegen 更慢、泛型实例化位置、整个开发周期的盈亏；不全仓直接 release |
| 较低构建并发／重测试分组 | 降低换页与 I/O 竞争 | 单个阶段与总吞吐的权衡；不能从某机器的数字导出全仓固定上限 |

Cargo test profile 默认继承 dev；环境与 config 还能覆盖 manifest，先查实际生效值。
Rust 1.90 起 x86_64-unknown-linux-gnu 默认使用 LLD，但其他目标与本机 override 仍须核对，
不能把重复设置默认值当成一项收益。

## 可比的实验设计

先确认实验问题：是热运行、改动后的反馈、首次构建，还是业务性能。不要把它们混为一项。
对每个候选复现同一变更转移与前置构建条件，不只测“同一命令再跑一次”的空转缓存命中。

建议选取：无源码变化的热重复、一个内部实现编辑、一次影响消费者的契约编辑、一次所需 feature
切换。每种条件记录 revision/diff、包与 feature 集合、profile、工具链、并发、缓存与存储。
对照顺序可交错为 A/B/B/A；涉及配置变化时，分别完成各自所需预热，区分预热、配置切换成本和
编辑成本。保留原始样本，给中位数与范围，不用最快一轮作结论。

不在共享 target 执行 cargo clean。严格冷启动实验需要明确授权和隔离的实验条件，不能为了
公平对照删除大家的缓存。磁盘页缓存、换页、其他进程与 Docker 卷／文件系统条件也要记录。
性能报告可留在任务或证据目录，不新增每项功能都必须维护的指标清单。

## 退出条件

只有热点证据支持且整条目标流程有收益时，才将候选变为默认。节省少量时间却增加全局可变状态、
平台依赖或维护成本时可以不合入。已经撤回的方案保留原因，不以中间成绩抵扣最终流程成本。
集成修复后重新核对选中范围和语义；本 skill 不授予 commit/push 或跑全仓测试的额外权限。

## 一手参考

- [Cargo FAQ：重建诊断](https://doc.rust-lang.org/cargo/faq.html#why-is-cargo-rebuilding-my-code)
- [Cargo features](https://doc.rust-lang.org/cargo/reference/features.html)
- [Cargo tree](https://doc.rust-lang.org/cargo/commands/cargo-tree.html)
- [Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html)
- [Cargo build：targets 与 timings](https://doc.rust-lang.org/cargo/commands/cargo-build.html)
- [Cargo build scripts：失效输入](https://doc.rust-lang.org/cargo/reference/build-scripts.html)
- [Rust 1.90：Linux 默认 LLD](https://blog.rust-lang.org/2025/09/18/Rust-1.90.0/)
- [nextest：测试分组](https://nexte.st/docs/configuration/test-groups/)

参考描述工具机制，不承诺本仓库的收益或某个工具版本永远不变。
