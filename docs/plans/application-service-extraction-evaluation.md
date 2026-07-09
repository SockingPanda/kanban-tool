# Application Service 抽取评估

状态：已被 application API 收敛迁移取代。当前 `kanban-application` 承载 DTO/port 合同，`kanban-sqlite::api`/`SqliteApplication` 是 SQLite-backed adapter boundary；crate root legacy re-export 已删除。

本文记录未来什么时候值得把独立 application-service crate 抽出来，以及一旦重启这项工作必须保住哪些 invariant。它是评估记录，不是当前迭代的实现计划。

## 当前形态

当前 workspace 只有一套共享 use-case implementation surface：

```text
kanban-cli
kanban-server handlers
apps/desktop src-tauri commands
dispatcher CLI path
        -> kanban-sqlite::service
              -> kanban-core pure state-machine helpers
              -> canonical SQLite truth
              -> events / outbox / dirty-generation markers
```

`kanban-core` 负责纯领域 helper，例如 `TaskStatus`、transition guard、ID/error/clock 类型和 readiness recompute 逻辑。它目前不拥有完整 command-service interface，也不拥有持久化 records。

`kanban-sqlite::service` 负责必须和 SQLite transaction 边界一起发生的 orchestration：board/task resolution、status guard、canonical writes、run/event rows、label 和 ontology provenance、import/doctor gate，以及 derived-store dirty markers。

Adapter 证据：

- `kanban-cli` 的 task、label、dispatch、search、maintenance、board、comment、run、index、context、serve 等命令调用 `kanban_sqlite::api`，init/runtime 基础设施走显式模块。
- `kanban-server` handlers 负责 HTTP DTO 转换，然后调用 `kanban_sqlite::api` 或 `kanban-application` wrapper 进入同一组 SQLite-backed use cases。
- `apps/desktop/src-tauri` 通过 `kanban_sqlite` 初始化 DB runtime，并通过同一 service layer 读取 boards。
- `kanban dispatch` 是 CLI 入口，调用同一 SQLite dispatch service；当前没有独立 dispatcher business-logic crate。

## 决策

已抽取 `kanban-application` DTO/port contract crate；不把 transaction implementation 从 `kanban-sqlite::service` 迁出。

当前最高风险仍在 label ontology mutation、validation evidence、derived-store dirty state、board isolation 和 adapter parity 的行为契约。行为契约稳定前移动 orchestration，会增加 churn，但不会天然带来新的安全保证。

因此架构文档应继续诚实描述当前形态：application contract 在 `kanban-application`，SQLite-backed implementation owner 仍在 `kanban-sqlite::service`，`kanban-core` 保持为纯领域 / 状态机 crate。

## 重新评估门槛

只有出现“匹配架构图”以外的具体收益时，才重新评估抽取。至少应满足以下条件之一：

- CLI、server、desktop 或 dispatcher 出现无法通过简单 shared helper 消除的 use-case orchestration 重复。
- adapter contract tests 发现不同入口之间存在行为漂移。
- 真实本地用例需要非 SQLite 的 application boundary，同时不引入第二数据库或 remote/SaaS 语义。
- mutation、validation、derived-store、import/doctor 和 state-transition contract 已稳定，移动 crate 边界不会同时改写活跃行为。

## 必须保留的 Invariants

未来如果抽取，必须保留这些边界：

- CLI、server、desktop 和 dispatcher 调用同一套 shared use-case implementation。
- `tasks.status` transition 继续经过同一组纯 state-machine helper 和同一条 transactional write path。
- SQLite transaction ownership 不在多个 crate 或 adapters 中复制。
- `ready -> running` 仍是一个 atomic claim transaction。
- `blocked -> ready` 仍重新计算 spec、schedule 和 dependency state。
- Derived stores 仍是可重建 projection，没有 canonical write path。
- Label ontology mutation 与 validation provenance 仍和它解释的 canonical rows 在同一 commit boundary 内提交。
- Adapter DTO 只做转换层，不变成 business logic。

## 未来倾向形态

如果抽取变得值得做，应围绕 use case 抽取，而不是围绕 table repository 抽取：

```text
adapters
  -> application service use-case API
        -> transaction/runtime port
        -> domain/state-machine helpers
        -> canonical persistence implementation
```

抽出的 boundary 必须显式表达 transaction scope。不要把每个 table operation 都包成贫血 repository interface，因为很多重要保证是跨 row、跨 table 的：status transition 加 event、claim 加 run、ontology action 加 atom mutation、import replacement 加 doctor consistency gate。

## 建议顺序

1. 先保持 `ARCHITECTURE.md` 对当前 crate 布局的真实描述。
2. 重构前先增加代表性 adapter contract tests：task status、label binding、semantics mutation 和 ontology action。
3. 稳定 label ontology mutation/validation 和 derived-store contracts。
4. 如果抽取仍有具体收益，先设计 use-case API 和 port boundary。
5. 先移动一个窄 vertical slice，并证明 CLI/API parity，再继续扩大。

## 非目标

- 不引入 Postgres、MySQL、MongoDB、remote service、SaaS 假设、RBAC、organizations、teams 或 multi-user 语义。
- 不为了让图更好看而拆 crate。
- 不在 adapters 中复制 status machine 或 SQLite transaction orchestration。
- 不把 derived-store write authority 移到 graph/vector/search 层。
