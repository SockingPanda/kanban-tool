# Release provenance

`just release` 只进入 `scripts/release-cohort.sh`。该 wrapper 是一次发布 cohort 的唯一
进程 owner；不要逐条手工执行其中的 gate 后拼装发布包。

## Source gate

`release-cohort.sh` 在读取 Git/source 状态前先持有一次共享 Cargo target 的 exclusive
build lock。wrapper 内所有 `just`、Cargo build、package、hash 和 publish 都通过
`KANBAN_CARGO_BUILD_LOCK_HELD=1` 复用同一把锁；锁不会在各 gate 之间释放。

构建开始前必须同时满足：

- 脚本所在目录是真实 Git root；
- HEAD 是 symbolic `main`，不能是 detached HEAD 或临时分支；
- tracked 与 untracked 工作树都为空；
- HEAD commit 与 tree 都是完整 40 位 object id；
- `git ls-remote origin refs/heads/main` 成功、只返回一个精确 ref，且 commit 与 HEAD
  完全一致。
- canonical source map 的 `source_tip`、保存的
  `refs/remotes/origin/derived-projection-v2` 和实时
  `git ls-remote origin refs/heads/derived-projection-v2` 三者完全一致；
- `0e58068`、`85e1f79`、`c764706` 三个 canonical source slice 都是上述保存 tip 的
  ancestor，三个 semantic-port commit 都是 `main` HEAD 的 ancestor；
- `main` HEAD 与保存的 derived source tip 确实没有 merge base。出现 merge base、
  Git 查询错误或不完整输出都会 fail closed。

通过后生成 deterministic build identity：

```text
kanban-tool/<version>;commit=<40sha>;tree=<40sha>;identity=<64sha>
```

source gate 生成 schema v3 的 canonical manifest。除 commit/tree 外，manifest 精确绑定
`Cargo.lock` SHA-256、批准的 `policy/schema-tool-registry-closure.json` SHA-256、完整
`rustc -vV` 与 `cargo --version`（各自 hash 及组合 toolchain identity）、effective release
features（当前为 `--no-default-features` + `tantivy-backend,oxigraph-backend`）、rustc target
triple、平台/机器架构和 Debian architecture。identity 对象和组合 hash 都拒绝缺字段、额外
字段、非法值、绝对路径与 cache 路径；manifest 不携带 secret。该值通过 `KANBAN_BUILD_ID`
传给同一 wrapper 启动的所有 Rust 构建。source gate 只允许把 machine manifest 写到 source
tree 之外的共享 Cargo target；它不会修改仓库文件。

这些字段同时是 effective build contract，而不是仅供展示的标签：release 固定
`--no-default-features --features tantivy-backend,oxigraph-backend`、manifest 中记录的
host target triple，以及采样时解析出的同一 `rustc`/`cargo` 可执行文件。wrapper 在每次
`just` 前后复核可执行文件和 `-vV`/`--version` 输出，随后以 `RUSTC`、`CARGO`、
`CARGO_BUILD_TARGET` 和 canonical feature/default-mode 环境调用 recipe；`cli-package` 与
`projection-release-cohort` 的 recipe 还必须在 immutable `justfile` 中保留完全相同的
Cargo 参数。外部的 `RUSTC_WRAPPER`、`RUSTC_WORKSPACE_WRAPPER`、`RUSTFLAGS`、target/profile/
registry/native compiler 变量（包括 `CARGO_TARGET_*`）和 `RUSTUP_TOOLCHAIN` 等未记录设置
一律 fail closed；共享 target 目录只由 build-lock 在取得锁后注入。

source gate 通过后，wrapper 从 pinned HEAD object 创建
`.cohort-source.<commit>-<tree>-<identity>/source.tar`，逐项拒绝绝对路径、`..`、link 和
特殊 tar member，再提取并 seal 为私有 source tree。所有 build/package recipe 从该 tree
的 `justfile`/`Cargo.toml` 运行；live worktree 只用于 source recheck、`diff-check` 和最终
证据读取，绝不作为构建输入。source archive 在 build 前再次触发 source gate；若 archive
期间 live commit/tree/remote/ancestry/no-merge-base 证据漂移，构建和 publish 都失败。仅
Desktop 的 `dist` 与 sidecar `binaries` 是预先声明的 snapshot output directories，其他
source entry 保持只读。
snapshot 生成后，Desktop provenance injector 与 artifact manifest gate 一律从该 sealed
source tree 运行；live worktree 中同名脚本的替换不会进入 cohort。

## Cohort evidence

共享 target 的
`release/bundle/cohort/<main-commit>-<tree>-<identity-sha256>/` 保存一个 immutable generation：

- `source-provenance.json`（schema v3）：version、commit、tree、generation key、完整 release
  identity、main 实时远端 tip、derived source
  的保存/实时精确 tip、no-merge-base 证明、三个已验证 source commit、build identity
  以及 source map hash；
- `release-artifacts.json`：CLI、desktop、LanceDB helper、Oxigraph helper 与两个 Debian
  包的 SHA-256、大小和 cohort identity。

generation 内的隐藏 `.release-tools/` 还保留与该 source snapshot 相同的 artifact
manifest/injector 副本，用于 crash/resume 时继续执行 sealed gate；live worktree 脚本不作为
恢复 fallback。

artifact manifest 的 `generation_path` 必须逐字节等于
`release/bundle/cohort/<generation_key>`；只匹配 basename 或任意安全 suffix 都不构成
cohort evidence。所有 artifact/source manifest/map path 都从这个 canonical publication
directory 推导，resume 与 sibling collision 也以完整 commit/tree/identity key 比较。

构建输出不会直接作为 cohort 证据。artifact gate 先把六个 artifact 复制到 mode `0700`
的同文件系统私有 staging generation。`release-safe-path.py` 从 public root 开始逐组件用
`O_NOFOLLOW` 打开并保留 dirfd；创建、复制、校验、rename、rollback 与 fsync 都只使用
dirfd + basename 的 `*at`/fd 操作，并在 mutation 边界重走 public path 比较
`(st_dev, st_ino)`。因此 parent symlink/rename ABA 即使在 identity check 后竞争，也只能
让操作 fail closed 或落在已锚定的原目录，不能逃逸到替换后的 symlink target。所有 tree
entry 只能是同盘 directory 或单链接 regular file；symlink、hardlink（`nlink != 1`）、
特殊文件、mount crossing、越界路径以及任一 xattr 都会被拒绝。零 xattr policy 明确包含
`user.*`、POSIX ACL 的 `system.posix_acl_*`、`security.capability` 和未知属性；无法枚举
xattr 也 fail closed。release target filesystem 因而必须不会自动给 cohort entry 注入
必须保留的 security xattr。

复制前后还会核验 source identity/size/time。hash 只针对 staged copy。每次创建 nested
directory/file 后都会 fsync 新对象与父目录；私有目录、copy、seal、durable tree、publish
parent 与 rollback parent 使用独立 checkpoint，seal 会对 chmod 后 metadata 与全部目录
自底向上 durable sync。source recheck 后 artifact gate 会重新解包 package、比较 payload、
执行两个 helper
的隐藏 `__build-identity` command 并要求 stdout byte-for-byte 等于
`KANBAN_BUILD_ID`，然后对全部 staged copy 复哈希。

release publish 只支持具备 Linux `F_SETLEASE` 与
`renameat2(RENAME_NOREPLACE|RENAME_EXCHANGE)` 的环境，任一 primitive 缺失、`ENOSYS`、
已有 writer 或 lease 状态不可读都没有 fallback；系统调用与 test fault injection 共用
`errno.ENOSYS` 分类和精确诊断分支。helper 对每个 staged regular file 获取 kernel read lease，
保留完整 pinned-fd tree 的 exact entry set、inode/type/link/mode/size/mtime/ctime 与零 xattr
snapshot；在 lease 内依次执行 source/artifact semantic verify、pinned-fd whole-tree digest、
dirfd atomic rename、parent fsync、published-path identity/digest 复验，再执行一次
post-publish semantic verify 与最终 digest。semantic verifier 不再从可被 ABA 替换的
public pathname 读取 generation：safe-path 把 source/destination 参数改写为继承的
`/proc/self/fd/<fd>` pinned tree，并把 exact `(st_dev, st_ino)` 交给 verifier；verifier
及其受控子进程只显式传播该 fd。verifier 运行期间会轮询每个 `F_GETLEASE`；
SIGIO 或任一 lease 不再是 `F_RDLCK` 会立即终止 verifier 并中止/回滚。内容 writer 在
transaction 内由 kernel 阻塞；lease 不阻止的 chmod、xattr、unlink/rename 则由 exact
snapshot 与 rename 后复验检测并回滚。相同 identity 已存在时失败，旧 generation 保留
不覆盖；最终 parent fsync 或 post-publish gate 失败会以同一 dirfd primitive 原子回滚，
回滚 parent 也必须 fsync，重试从恢复后的 private stage 收敛。

generation 本身的 rename 不是 commit boundary。generation name 同时是 commit/tree 与
toolchain/dependency/features/target identity 的 resume key；相同 commit/tree 下发现旧
schema、旧 identity 或 legacy commit-tree generation 会 fail closed，不会复用或覆盖。rename
前先 durable 创建同级 `<generation>.publishing` intent；它精确绑定 deterministic source-stage
name、generation name（因此包含完整 identity）、tree digest 与 tree `(st_dev, st_ino)`。只有
source 缺失、destination 未标记且
intent/tree/inode/digest 全部匹配时，recovery 才能用 retained dirfd 把 destination 恢复回
source；无 intent、内容不匹配或未知 inode 一律原地保留。最后一次 post-publish
semantic/digest、lease、inode 和零 xattr 检查完成后，helper 才把同一 intent 以
`RENAME_NOREPLACE` 发布为 `<generation>.published` marker。读者必须先取得
generation/marker 的 pinned fd 与 read lease，
逐字节验证 marker、exact tree 和 digest，才可采用该 generation。marker/intent 的
destination key 与 generation name 一致，因此不会把同一 commit/tree 的另一 toolchain、lock、
feature、target 或 arch 当作同一 cohort。无 marker 的 generation 不具 authority。wrapper 先
用临时 source-identity bootstrap 确定 generation，再使用
`.cohort-stage.<commit>-<tree>-<identity-sha256>` deterministic stage；若进程在 generation rename 后、
marker 前崩溃，完整 wrapper 重跑会在 existing-generation 拒绝检查之前按 durable intent
恢复该 stage，重新执行 source/artifact/digest gate 后继续 publish。成功路径一直保留
tree/marker leases 与 pinned fds，完成 stdout/stderr flush 后直接退出进程，避免在最终
检查与 lease 释放之间留下可继续执行的 userspace commit window。

这里的 immutable generation 是发布 transaction 与 no-replace/history contract，不宣称
lease 释放后能抵抗拥有 filesystem 管理权限的 hostile operator。成功后仍须用生产权限
边界保护 cohort root；任何后续 out-of-band chmod/xattr/content/entry 修改都属于证据篡改，
不能把 `0555/0444` 当作永久 kernel immutability。

CLI 与 desktop Debian 包还分别内嵌完全相同的 `source-provenance.json` 和
`derived-projection-v2-source-map.json`。artifact gate 会解包并逐字节比较 CLI、desktop
与两个 helper，拒绝旧包、混合 cohort、缺少/伪造 runtime build identity 或 provenance
不一致。CLI package 与 desktop provenance injector 都在输出目录内使用私有同盘 staging，
并以 `RENAME_EXCHANGE` 条件替换单链接 regular file；旧 target 会暂留在已锚定的私有
source stage 作为 rollback copy，直到 destination identity、parent fsync、lease 与零
xattr 最终检查全部通过，再按预先记录的 directory `(st_dev, st_ino)` 由 dirfd cleanup
删除。destination 在检查后发生 ABA、已有 symlink/hardlink target、parent fsync 失败或
rollback parent 无法 durable 时都 fail closed，不会覆盖未知 entry。commit 前还会重走
public source/destination parent 并核对 destination exact inode；rollback 前必须同时证明
exchange 两端仍分别是 retained old/new inode。任一端 drift 时不再 exchange，并在 private
stage 写入 durable retention marker，使后续 identity-bound cleanup 拒绝递归删除。

`docs/release/derived-projection-v2-source-map.json` 是无 merge-base 集成的 canonical
来源映射。它明确记录 `origin/derived-projection-v2` 三个已推送切片与语义移植 commit；
不要用普通 merge 改写这段来源关系。

所有 gate、打包与首次 hash 完成后，wrapper 会再次执行完整 source gate，并把当前状态与
最初 manifest 做 byte-for-byte 比较；随后复验 package/runtime binding 与每个 staged hash，
最后才原子发布整个 generation。构建期间出现 dirty/untracked、branch、HEAD、tree、
main/derived remote、保存 tip、source ancestry、merge-base 或 staged artifact 漂移时，
发布失败，既有 generation 不受影响。只有仍与预先 pinned identity 相同的私有临时 tree
才会通过 dirfd 递归清理；遇到 identity drift 或无法证明安全删除时保留 failed staging
并输出路径，交由 operator 按证据处置，不对未知 pathname 执行递归 chmod/rm。

release wrapper 本身由 whole-file SHA-256 与结构化 `publish_generation` argv graph
共同锁定；dead branch、environment skip、命令新增/删除/重排都会使 witness 失败。
hermetic wrapper 测试使用 default-deny `PATH`，只暴露声明过的工具，并把 build-lock、
Git、`just`、Debian tooling 与 helper runtime identity 调用写入统一 JSONL；规范化后的
311 行 ordered trace 以 exact SHA-256 比较，额外 direct `cargo` 或 host `PATH` fallback
都会失败。
