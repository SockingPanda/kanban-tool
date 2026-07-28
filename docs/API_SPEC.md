# 本地 Web API 规范

本 API 只面向 Tauri Desktop 和本地脚本。它不是远程协作 API。

独立运行 `kanban serve` 时默认监听：

```text
127.0.0.1:8721
```

Tauri 内嵌运行时绑定 `127.0.0.1:0`，由操作系统选择可用端口。

基础路径：

```text
/api/v1
```

---

## 1. 通用约定

### 1.1 内容类型

请求：

```http
Content-Type: application/json
```

响应：

```http
Content-Type: application/json
```

SSE：

```http
Content-Type: text/event-stream
```

### 1.2 操作者

因为没有多用户系统，`actor` 是审计字段。

来源优先级：

1. 请求体中的 `actor`。
2. 请求头 `X-KB-Actor`。
3. 服务端默认的 `actor`。
4. 操作系统用户名。

#### 1.2.1 请求头契约

除 SSE 事件流外，当前 83 个 HTTP 端点都拥有端点专属、精确且
`deny_unknown_fields` 的请求头契约。每份契约都包含可选的 `Accept-Language`，并按处理器的真实
输入选择语言、语言加操作者、语言加 JSON 内容类型，以及它们允许省略请求体的变体。
`X-KB-Actor` 只出现在会解析操作者的变更处理器中。

必须提供 JSON 请求体的端点要求且只允许一个 `Content-Type`；允许省略请求体的归档、
推进、回收、解除阻塞，以及标签提议、接受和拒绝端点将其建模为可选；没有
请求体的端点不声明 `Content-Type`。这些数量约束属于传输契约，不改变 Axum
对具体媒体类型和格式错误 JSON 的既有 400 行为。

SSE 的 `Last-Event-ID` 仍明确标记为 `Excluded`：当前运行时忽略该请求头，没有续传契约；
不得因为其他端点已经收紧请求头，就把它推断为已采用的输入。

### 1.3 成功响应

成功响应按端点的元数据契约使用以下线上封装：

`DataEnvelope` 仅包含 `data`：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {}
}
```

`MetadataEnvelope` 的 `meta` 是必需字段：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {},
  "meta": {}
}
```

`OptionalMetadataEnvelope` 只在端点产生对应元数据时包含 `meta`；没有元数据时
直接省略该字段，不返回 `"meta": null`。具体端点使用哪一种封装及其
`meta` 字段，由该端点的响应示例和说明定义。

### 1.4 错误响应

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "error": {
    "code": "invalid_transition",
    "message": "cannot claim task from status todo"
  }
}
```

`error.code` 是稳定的机器契约。`error.message` 是供人阅读的文案，会根据
`Accept-Language` 在 `zh-CN` 和 `en` 之间选择；未传请求头时保持既有默认值 `en`。
客户端逻辑必须读取 `error.code`，不要解析 `error.message`。

### 1.5 HTTP 状态码映射

| 错误代码 | HTTP 状态码 |
|---|---:|
| `invalid_input` | 400 |
| `not_found` | 404 |
| `conflict` | 409 |
| `dependency_cycle` | 409 |
| `invalid_transition` | 409 |
| `dependency_blocked` | 409 |
| `execution_plan_required` | 409 |
| `steps_incomplete` | 409 |
| `claim_conflict` | 409 |
| `claim_token_mismatch` | 403 |
| `internal` | 500 |

---

## 2. 健康检查

### `GET /health`

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "ok": true,
    "db": "ok",
    "version": "2.1.3",
    "db_path": "/home/alice/.local/share/kb/kb.db",
    "db_fingerprint": "sqlite:131072:1717520000000"
  }
}
```

`db_path` 和 `db_fingerprint` 让本地桌面端或 Web 开发界面能够确认由哪一个
SQLite 运行实例响应了请求。若配置的数据库文件已被删除，`/health` 会返回
`400 invalid_input`，而不是重新创建空的 SQLite 文件。其他 API 路由也会在运行处理器前
执行相同的文件缺失检查，因此过期或已删除的运行实例会明确失败，不会在配置路径上打开
新的空数据库。`/health` 还会验证数据库是否具备预期的迁移后结构；空数据库或未初始化的
SQLite 文件同样返回 `400 invalid_input`。

---

## 3. 看板

### 3.1 列出看板

```http
GET /api/v1/boards?include_archived=false
```

默认隐藏已归档看板；传入 `include_archived=true` 可将其一并返回。

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": [
    {
      "id": "b_01HX...",
      "slug": "default",
      "name": "默认看板",
      "description": null,
      "created_at": 1717520000000,
      "updated_at": 1717520000000,
      "archived_at": null
    }
  ]
}
```

### 3.2 创建看板

```http
POST /api/v1/boards
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "slug": "agent-work",
  "name": "代理工作",
  "description": "本地代理任务看板",
  "actor": "alice"
}
```

成功时返回 `201 Created`。看板 slug 必须唯一且非空，长度不超过 64 字节；首字符必须是
小写 ASCII 字母或数字，后续只能包含小写 ASCII 字母、数字、`.`、`_`、`-`，且不能以
`b_`、`t_`、`r_`、`c_`、`a_`、`l_`、`col_`、`e_` 等保留 ID 前缀开头。slug 重复或
格式无效时返回标准 `400 invalid_input` 错误封装，而不是 `500`。

### 3.3 获取看板

```http
GET /api/v1/boards/{board_slug_or_id}
```

### 3.4 归档看板

```http
POST /api/v1/boards/{board}/archive
```

归档会设置 `archived_at` 并写入 `board.archived` 事件，但不会修改任务。若看板上存在
活跃的 `running` 任务或仍在运行的任务执行记录，操作会返回 `409 invalid_transition`。
归档后，该看板上的普通任务变更会被拒绝；显式指定任务或看板标识时，审计历史端点仍可读取。

### 3.5 看板端点的精确契约

四个看板端点使用端点专属的契约根：列表查询、创建请求、获取或归档路径，以及各自的成功响应。
四个成功响应只共享闭合的 `ApiBoard` 组件；服务端会把 SQLite 应用记录显式映射为线上 DTO，
不会直接序列化 `BoardRecord`。归档请求体继续复用既有的 `ArchiveBoardRequest` 契约。

`include_archived` 默认为 `false`，传入 `true` 时会真实转发给服务层并返回已归档看板。
桌面端的 `listBoards` 调用方会精确校验 `data` 封装和 `ApiBoard` 的全部字段；字段缺失、
类型错误或出现额外字段时返回 `invalid_response`。运行中工作项的归档保护、已归档看板的
审计历史、未找到时的状态码和错误代码，以及依赖语言的消息文案不属于模式文件的权威范围，
继续由服务层和适配器保证。四个端点的路径、查询、请求头、请求体和成功响应契约均已采用，
迁移状态为 `Adopted`。

---

## 4. 任务

### 4.1 列出任务

```http
GET /api/v1/boards/{board}/tasks
```

查询参数：

| 参数 | 说明 |
|---|---|
| `status` | 可重复：`?status=ready&status=running`。 |
| `priority` | 可重复：`?priority=0&priority=2`，值为 P0-P3 的 `0..3`。P0 表示事故、阻塞项或必须立即处理的任务；P3 是普通待办、低优先级和默认值。 |
| `assignee` | 按执行者过滤。 |
| `label` | 按标签名称或 ID 过滤，可重复；多个标签使用 AND 语义。 |
| `plan_filter` | 可重复：`plan_needed` / `has_steps` / `incomplete_required_steps`。 |
| `q` | 搜索标题或描述；任务引用形状按精确匹配处理。 |
| `include_archived` | 布尔值。 |
| `limit` | 默认 100。 |
| `offset` | 分页偏移量。 |
| `sort` | `seq` / `title` / `status` / `position` / `priority` / `assignee` / `scheduled_at` / `due_at` / `created_at` / `updated_at`，前缀 `-` 表示降序。`priority` 按 P0 到 P3 排序；`-priority` 按 P3 到 P0 排序。 |

这两个任务读取端点使用同一套严格的原始查询语法，但各自拥有独立的精确路径与查询契约，
并由两个服务端本地的强类型 Axum 提取器分别绑定真实 `{board}` 路径和唯一的原始 URI
查询消费点；处理器只接收已解析请求，不持有 `RawQuery` 或第二套 `Query<T>` 提取器。
只有 `status`、`priority`、`label`、`plan_filter` 可以重复；不同语义值按 URI 首次出现顺序
保留，任何重复语义值返回 `400 invalid_input`。`assignee`、`q`、`include_archived`、
`limit`、`offset`、`sort` 任一重复也返回 `400`。任何未知 key 失败关闭；旧 `search`
alias 已删除，只接受 `q`。

原始查询最多 8192 字节。参数对上限不是独立字面量，而是由 9 个 `status`、4 个
`priority`、3 个 `plan_filter`、32 个 `label` 和 6 个标量参数推导出的 54。
`q` 最多 1024 个 Unicode 字符，`assignee` 与单个 `label` 最多 128 个。未提供查询时
默认 `include_archived=false`、`limit=100`、`offset=0`、`sort=position`。`limit` 的线上
权威上限是 `kanban-contract` 的 1000；SQLite 服务层的防御性上限直接引用唯一的应用权威值，
服务端对这条实际服务路径建立编译期相等门禁。`offset` 最大为
`i64::MAX`。空的 `q`、`assignee` 归一化为未提供；label 会规范化 Unicode 边缘空白，但必须
包含至少一个非空白字符，且 raw 字符长度不得超过 128；该预算在 trim 前计算，随后会被移除
的 Unicode 边缘空白也计入 128 字符。空或纯 Unicode 空白 `label`、enum、bool、数字或 sort
值无效。
查询使用严格的表单解码：`+` 表示空格，`%HH` 必须完整且解码结果必须是 UTF-8；合法
UTF-8 与 `&`、`/`、`=`、`+`、空格必须由标准表单编码器转义，非法百分号编码
或 UTF-8 返回 `400`。

优先级只表达相对重要性和排序，不表示能否认领。`ready` 才表示任务已显式进入可执行队列；
普通 `ready` 任务可以是 P1、P2 或 P3，不应为了表示“可做”而全部标成 P0。P0 只用于事故、
当前目标阻塞项或必须立即处理的任务；P0 任务若仍缺规格、排期未到或依赖未完成，仍不能被认领。

`q` 对任务引用形状使用精确匹配，而不是文本包含匹配：纯数字 `12` 和
`#12` 匹配 `{board}` 内的序号；`board#12` / `board/#12` 只在显式看板
与 `{board}` 相同时匹配；`t_...` 只匹配 `{board}` 内的任务 ID。其他文本仍执行
标题和描述的模糊搜索。

响应（以下为字段节选；完整、可消费的成功响应以 `schemas/fixtures/api/list-tasks-response.v1.valid.json` 为准）：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": [
    {
      "id": "t_01HX...",
      "seq": 12,
      "board_id": "b_01HX...",
      "board_slug": "agent-work",
      "ref": "agent-work#12",
      "title": "实现状态机",
      "description": "...",
      "status": "ready",
      "priority": 1,
      "position": 1024,
      "assignee": null,
      "scheduled_at": null,
      "due_at": null,
      "created_at": 1717520000000,
      "updated_at": 1717520000000,
      "labels": [
        {
          "id": "l_01HX...",
          "board_id": "b_01HX...",
          "name": "core",
          "color": null
        }
      ],
      "dependency_blocked": false,
      "unfinished_parent_count": 0
    }
  ],
  "meta": {
    "limit": 100,
    "offset": 0,
    "total": 1
  }
}
```

### 4.2 创建任务

```http
POST /api/v1/boards/{board}/tasks
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "title": "实现状态机",
  "description": "Markdown 规格",
  "status": "ready",
  "assignee": "local-worker",
  "priority": 1,
  "scheduled_at": null,
  "due_at": null,
  "max_retries": 2,
  "depends_on": ["t_01HX..."],
  "labels": ["core"],
  "metadata": {},
  "actor": "alice"
}
```

说明：

- `status` 只能是 `triage|todo|scheduled|ready`。
- 若不传 `status`，服务端计算初始状态。
- 显式请求 `scheduled` 时必须同时提供 `scheduled_at`。
- 显式请求 `ready` 时必须有非空描述，且 `scheduled_at` 不能位于未来。
- 若存在未完成依赖（父任务不是 `done` 或 `archived`），`status=ready` 请求仍可接受，
  但依赖守卫会让最终状态保持为 `todo`。
- 无论显式请求 `ready`，还是省略 `status` 后计算出 `ready`，新任务尚无执行计划时
  都会实际以 `todo` 状态落库；响应会把 `execution_plan_state` 派生为 `unplanned`，
  不会为此写入计划行。添加第一个步骤，或通过
  `/execution-plan/not-required` 明确标记无需计划并填写原因后，服务端才会结合规格、
  排期和依赖等其他保护条件重新计算是否进入 `ready`。
- 任务响应会公开派生的依赖和执行计划字段：`dependency_blocked`、`unfinished_parent_count`、
  `execution_plan_state`，以及必需或可选步骤数量。它们是查询元数据，不是可写任务字段。
- `priority` 是整数等级 `0..3`：`0` = P0 事故、阻塞项或必须立即处理，`1` = P1 近期重点，
  `2` = P2 重要后续，`3` = P3 普通待办、低优先级和默认值。创建时会拒绝非法值。
- `labels` 可选。名称会先去除两端空白；空白名称会被拒绝；所有标签必须已存在于当前看板。
  任一标签缺失时，整个创建请求返回 `400 invalid_input`，且不会写入 `tasks`、`labels`、
  `task_labels` 或 `task_events`。创建任务不提供自动创建缺失标签的模式。
- `priority` 默认为 `3`，`labels` 和 `depends_on` 默认为空数组；其他可空字段可显式传入
  `null`。`metadata` 只接受 JSON 对象或 `null`，对象内容是开放扩展，不在传输层解释。
- 路径、请求与 `201` 成功响应分别由 `CreateTaskPath`、`CreateTaskRequest` 和
  `CreateTaskResponse { data: ApiTask }` 拥有。请求状态使用仅限创建的闭合词汇
  `triage|todo|scheduled|ready`；公开响应不包含 `claim_token`。
- 处理器只负责把契约显式映射到应用输入，并继续单次调用
  `create_task_with_labels_and_dependencies`。标签、依赖、重试策略、元数据有效性和初始就绪
  判断仍在同一 SQLite 事务和服务保护中处理；任一失败都会整体回滚。

### 4.3 按状态列出任务窗口

```http
GET /api/v1/boards/{board}/tasks/by-status?status=triage&status=ready&include_archived=false&limit=50&offset=0&sort=-updated_at
```

这个只读端点把看板列查询合并为一次请求，并接受 4.1 节定义的同一套严格查询语法。
每个重复的 `status` 生成独立任务窗口；`limit` 与 `offset` 分别应用到每个窗口。
响应中的状态顺序与 URI 中重复参数的顺序一致；省略 `status` 时返回空的 `statuses` 数组。

响应（以下为字段节选；完整、可消费的成功响应以 `schemas/fixtures/api/list-tasks-by-status-response.v1.valid.json` 为准）：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "statuses": [
      {
        "status": "ready",
        "tasks": [
          {
            "id": "t_01HX...",
            "ref": "default#12",
            "status": "ready",
            "title": "实现状态机"
          }
        ],
        "page": {
          "limit": 50,
          "offset": 0,
          "total": 3
        }
      }
    ]
  },
  "meta": {
    "limit": 50,
    "offset": 0
  }
}
```

### 4.4 获取任务

```http
GET /api/v1/tasks/{task_id}
```

`task_id` 是全局 `t_...` ID，不受看板作用域限制。响应包含 `board_id`、`board_slug`
和 `ref`，便于客户端展示可复制的 `board#seq` 任务引用。

查询参数：

| 参数 | 说明 |
|---|---|
| `include` | 可选。当前识别 `ontology`；可用逗号分隔，其他 include 值暂时保持兼容性忽略。 |

默认响应只包含 `data: ApiTask`，不返回 `meta`。传 `include=ontology` 时，`data`
保持同一 `ApiTask`，并在 `meta.details.ontology_summary` 返回该任务的标签本体信号摘要；
没有本体信号时为 `null`。该摘要是任务级的只读工作流提示，包含信号、状态、降级、过期和
操作数量，最早的开放或已确认信号时间及距今时长，最新信号或操作时间，当前
`suggest_input_hash`，以及最多 5 条示例信号
（ID、种类、状态、提议操作、分数、过期、降级和操作数量）。完整队列和审核仍使用
`/label-ontology/signals`、`/label-ontology/review` 和
`/label-ontology/signals/{signal_id}`。

当前 API 不包含
`GET /api/v1/tasks/{task_id}/detail?include=dependencies,steps,runs,events,comments,neighborhood`
这类任务详情聚合端点，也不包含面板专属时间线。现有的分面板路由和缓存失效行为稳定后，
可再考虑以聚合端点减少 `TaskDetail` 面板的请求扇出。任务执行上下文已有独立的
`GET /api/v1/tasks/{task_id}/context` 端点，见 4.5 节。

### 4.5 获取任务上下文

```http
GET /api/v1/tasks/{task_id}/context?board=default&lexical_limit=5&graph_limit=10&vector_limit=5&max_items=20
```

这是已经采用精确路径、查询、请求头和成功响应契约的只读端点，迁移状态为 `Adopted`。
`task_id` 为必填路径参数；
查询参数均只能出现一次，未知参数会被拒绝：

| 参数 | 是否必填 | 默认值 | 说明 |
|---|---|---:|---|
| `board` | 否 | `default` | 看板 slug 或 ID。 |
| `lexical_limit` | 否 | `5` | 词法检索条目上限，范围 `0..=1000`。 |
| `graph_limit` | 否 | `10` | 图关系条目上限，范围 `0..=1000`。 |
| `vector_limit` | 否 | `5` | 向量检索条目上限，范围 `0..=1000`。 |
| `max_items` | 否 | `20` | 合并后的上下文条目总上限，范围 `1..=1000`。 |

响应的 `data` 包含 `subject`、回显实际限制的 `policy`、合并后的 `items`、
降级原因 `degraded`，以及有诊断时才出现的 `diagnostics`。图或向量后端不可用时，
端点会保留可用来源的结果并通过这些结构化字段说明降级，不会把派生存储当作权威数据源。

### 4.6 更新任务字段

```http
PATCH /api/v1/tasks/{task_id}
```

允许字段：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "title": "新的标题",
  "description": "新的描述",
  "assignee": "worker-a",
  "priority": 1,
  "scheduled_at": 1717520000000,
  "due_at": 1717600000000,
  "max_retries": 2,
  "metadata": {},
  "actor": "alice",
  "expected_lock_version": 7
}
```

`priority` 更新会拒绝 `0..3` 以外的值。

`max_retries: null` 会清空重试策略。任务 DTO 包含 `execution_plan_state`、
`required_step_count`、`completed_required_step_count` 和 `optional_step_count`，
因此客户端无需另行列出步骤，也能展示执行计划是否就绪。

禁止字段：

- `status`
- `claim_token`
- `current_run_id`
- `completed_at`

`PATCH` 不能直接设置规范 `status`；状态必须通过状态转换端点修改。允许字段仍会走共享服务路径。
更新 `description`、`scheduled_at` 等影响规格或排期的字段后，服务端可以根据规格、排期和
当前依赖重新计算活跃任务的目标状态，并写入对应事件。依赖边必须通过依赖端点修改；
`max_retries` 只更新重试策略，不会触发状态重算。

---

## 5. 状态转换

状态转换请求使用各端点独立的封闭 DTO，未知顶层字段会导致 `400`，不共享通用的转换或
令牌请求体。推进、回收认领、解除阻塞和任务归档可以完全省略请求体；出现请求体时仍按对应
DTO 校验。`actor` 的解析优先级保持为请求体、`X-KB-Actor`、服务端默认值。认领和心跳省略
`ttl_ms` 时使用 `300000`；回收认领、完成、提交审核、阻塞和归档省略 `force` 时均为
`false`，不能绕过租约、令牌或状态机保护。认领令牌不匹配的响应不会回显客户端提交的错误
令牌，也不会暴露服务端保存的真实令牌。

### 5.1 补全规格

```http
POST /api/v1/tasks/{task_id}/transitions/specify
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "description": "补全后的规格",
  "scheduled_at": null,
  "actor": "alice"
}
```

### 5.2 推进

```http
POST /api/v1/tasks/{task_id}/transitions/promote
```

任务执行计划仍为 `unplanned` 时，推进操作会返回 `409 execution_plan_required`。

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": "local-worker"
}
```

### 5.3 认领并开始

```http
POST /api/v1/tasks/{task_id}/transitions/claim
```

任务执行计划仍为 `unplanned` 时，认领并开始操作会返回 `409 execution_plan_required`。

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": "alice",
  "ttl_ms": 300000,
  "worker_profile": null,
  "metadata": {}
}
```

省略 `worker_profile` 或传入 `null` 时，运行时会把本次执行记录的工作进程配置记为
`"manual"`。

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task": {
      "id": "t_01HX...",
      "status": "running"
    },
    "run": {
      "id": "r_01HX...",
      "status": "running"
    },
    "claim_token": "claim_01HX...",
    "claim_expires_at": 1717520300000
  }
}
```

### 5.4 心跳

```http
POST /api/v1/tasks/{task_id}/transitions/heartbeat
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "claim_token": "claim_01HX...",
  "ttl_ms": 300000,
  "note": "仍在执行",
  "actor": "worker-default"
}
```

显式心跳仍受支持。对于 `running` 任务，后续合法且属于该任务的活动事件也会刷新任务租约和
当前执行记录的心跳，作为隐式存活信号；这种隐式续期不会额外产生 `task.heartbeat` 事件。
看板级事件和不含 `task_id` 的事件不会续期任务。

### 5.5 完成

```http
POST /api/v1/tasks/{task_id}/transitions/complete
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "claim_token": "claim_01HX...",
  "summary": "实现完成，测试通过",
  "result": {},
  "force": false,
  "actor": "worker-default"
}
```

`result` 是可选的不透明 JSON 值；schema 只约束字段存在形式，不收紧其内部结构。

### 5.6 提交审核

```http
POST /api/v1/tasks/{task_id}/transitions/submit-review
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "claim_token": "claim_01HX...",
  "summary": "等待人工检查",
  "force": false,
  "actor": "worker-default"
}
```

提交审核不接受 `result`；该字段与其他未知顶层字段一样会导致 `400`。

### 5.7 阻塞

```http
POST /api/v1/tasks/{task_id}/transitions/block
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "reason": "等待 API schema 确认",
  "claim_token": null,
  "force": false,
  "actor": "alice"
}
```

### 5.8 解除阻塞

```http
POST /api/v1/tasks/{task_id}/transitions/unblock
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": "alice"
}
```

响应中的目标状态由服务端计算，不由客户端指定。

### 5.9 重新打开

```http
POST /api/v1/tasks/{task_id}/transitions/reopen
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "reason": "重新执行修正验证失败",
  "actor": "alice"
}
```

只允许重新打开 `done` 任务，`reason` 必填且不能为空。响应中的目标状态由服务端按规格、
排期、依赖和执行计划就绪情况重新计算；`completed_at` 会被清空，`result_summary` 和自然
JSON `result` 会保留（持久层仍存于 `result_json`）。`task.reopened` 事件的载荷包含
`from`、`to`、`reason` 和 `original_completed_at`。

直接依赖该任务的子任务中，仅 `triage|todo|scheduled|ready` 会重新计算；
`running|blocked|review|done|archived` 不会被隐式改写。

### 5.10 回收认领

```http
POST /api/v1/tasks/{task_id}/transitions/reclaim
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "force": false,
  "to_status": "ready",
  "reason": "认领已过期",
  "actor": "local-worker"
}
```

`to_status` 是封闭枚举，只接受 `ready` 或 `blocked`；省略时默认为 `ready`，其他任务
状态会导致 `400`。目标为 `blocked` 时必须提供非空 `reason`。领取尚未过期时，只有
`force=true` 才能回收。

### 5.11 归档

```http
POST /api/v1/tasks/{task_id}/transitions/archive
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "force": false,
  "actor": "alice"
}
```

---

## 6. 依赖与执行计划

### 6.1 添加依赖

```http
POST /api/v1/tasks/{child_task_id}/dependencies
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "parent_task_id": "t_01HX...",
  "actor": "alice"
}
```

插入新依赖边时返回 `201 Created`。重复添加同一父子依赖是幂等操作，会以相同的依赖封装
返回 `200 OK`；不会再次写入 `dependency.added` 事件，也不会重复计算子任务状态。
依赖变化可能把不再合法的 `ready` 子任务降为 `todo`，但不会自动把 `todo` 子任务推进到
`ready`。重新打开 `done` 父任务时，仅当直接子任务处于
`triage|todo|scheduled|ready` 才会重新计算；`running|blocked|review|done|archived`
子任务保持不变。

### 6.2 移除依赖

```http
DELETE /api/v1/tasks/{child_task_id}/dependencies/{parent_task_id}
```

### 6.3 列出依赖

```http
GET /api/v1/tasks/{task_id}/dependencies
```

添加、移除和列出依赖的端点返回同一种依赖封装。在现有线上结构中，`parents` 和
`children` 是完整的 `ApiTask` 数组；额外的 `task` 和 `edges` 字段提供紧凑且已展开的
关系视图，其中父子对象使用稳定命名。

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task": {
      "id": "t_child",
      "board_id": "b_default",
      "board_slug": "default",
      "ref": "default#2",
      "title": "子任务",
      "status": "todo"
    },
    "parents": [],
    "children": [],
    "edges": [
      {
        "parent": {
          "id": "t_parent",
          "board_id": "b_default",
          "board_slug": "default",
          "ref": "default#1",
          "title": "父任务",
          "status": "done"
        },
        "child": {
          "id": "t_child",
          "board_id": "b_default",
          "board_slug": "default",
          "ref": "default#2",
          "title": "子任务",
          "status": "todo"
        }
      }
    ]
  }
}
```

### 6.4 步骤与执行计划

步骤是归属于任务的有序执行计划项。步骤可以是纯文本，也可以链接一个现有普通任务作为
上下文。链接任务不等于建立依赖边：链接不会影响依赖就绪判断，链接任务的状态也不会自动
完成该步骤。步骤完成状态通过 `todo | done | skipped` 独立跟踪。

```http
GET /api/v1/tasks/{task_id}/steps
POST /api/v1/tasks/{task_id}/steps
PATCH /api/v1/tasks/{task_id}/steps/{step_id}
DELETE /api/v1/tasks/{task_id}/steps/{step_id}
POST /api/v1/tasks/{task_id}/steps/{step_id}/done
POST /api/v1/tasks/{task_id}/steps/{step_id}/skip
POST /api/v1/tasks/{task_id}/steps/{step_id}/reopen
POST /api/v1/tasks/{task_id}/execution-plan/not-required
```

创建请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "title": "编写验收检查",
  "body": "覆盖依赖和执行计划保护",
  "linked_task_ref": "default#13",
  "position": 2048,
  "required": true,
  "actor": "alice"
}
```

`linked_task_ref` 可选；纯文本步骤应省略它。提供时，它必须解析到同一看板上未归档的任务，
且不能指向父任务本身。

更新请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "title": "编写验收检查",
  "body": null,
  "linked_task_ref": "default#14",
  "unlink_task": false,
  "position": 4096,
  "required": false,
  "actor": "alice"
}
```

步骤状态请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "note": "已经实现并验证",
  "actor": "alice"
}
```

`skip` 和 `reopen` 使用相同的封装，但文本字段名为 `reason`。

标记为不需要执行计划的请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "reason": "只是一次小型文本整理",
  "actor": "alice"
}
```

步骤列表和变更响应都会返回父任务的步骤快照：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task_id": "t_parent",
    "steps": [
      {
        "id": "step_01HX...",
        "parent_task_id": "t_parent",
        "title": "编写验收检查",
        "body": "覆盖依赖和执行计划保护",
        "linked_task": { "id": "t_child", "ref": "default#13" },
        "position": 2048,
        "required": true,
        "status": "todo",
        "resolution_note": null,
        "resolved_by": null,
        "resolved_at": null,
        "created_by": "alice",
        "created_at": 1717520000000,
        "updated_by": "alice",
        "updated_at": 1717520000000
      }
    ],
    "execution_plan": {
      "board_id": "b_01HX...",
      "task_id": "t_parent",
      "state": "planned",
      "reason": null,
      "updated_by": "system",
      "updated_at": 0
    }
  }
}
```

`POST /execution-plan/not-required` 直接返回执行计划记录。链接目标不存在时返回
`404 not_found`；链接自身、跨看板链接、链接已归档任务或标题为空时，以标准错误封装返回
`400 invalid_input`。完成或归档仍有必需步骤未完成的父任务时返回
`409 steps_incomplete`。对这项保护而言，必需步骤只有在状态为 `done` 或 `skipped`
时才算完成。

### 6.5 任务邻域

```http
GET /api/v1/tasks/{task_id}/neighborhood?depth=1&limit_nodes=250&include_archived_context=false
```

这个只读端点返回选中的任务、直接依赖父任务、直接依赖子任务、直接步骤链接的父子任务，
以及起点和终点都可见的每一条依赖边或步骤边。V1 只接受 `depth=1`；更深的图展开留待以后实现。

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "center_task_id": "t_01HX...",
    "nodes": [
      {
        "task": { "id": "t_01HX...", "ref": "default#12", "status": "ready" },
        "role": "center",
        "context_only": false
      }
    ],
    "edges": [
      {
        "id": "dependency:t_parent->t_child",
        "source_task_id": "t_parent",
        "target_task_id": "t_child",
        "kind": "dependency",
        "required": true,
        "blocking": true
      }
    ],
    "meta": {
      "depth": 1,
      "context_depth": 0,
      "node_count": 1,
      "edge_count": 0,
      "truncated": false,
      "limit_nodes": 250,
      "include_archived_context": false
    }
  }
}
```

`task` 与任务列表和详情响应使用相同的公开任务 DTO，不会暴露 `claim_token`。

### 6.6 看板任务图

```http
GET /api/v1/boards/{board}/task-map?active_only=true&context_depth=1&limit_nodes=250&include_done_context=true&include_archived_context=false&hide_isolated=false
```

这个只读端点返回看板的工作关系图。默认包含所有活跃且未归档的任务
（`triage`、`todo`、`scheduled`、`ready`、`running`、`blocked`、`review`），以及最多一跳的
未归档依赖上下文。默认包含 `done` 上下文并标记为 `context_only`；只有显式请求时才包含
已归档上下文。V1 只接受 `context_depth=0` 或 `context_depth=1`。

活跃看板任务的节点角色为 `active`，一跳上下文的角色为 `context`。只有边的两个端点都可见时
才返回依赖边和步骤边。依赖边使用 `kind=dependency`、`required=true` 和 `blocking=true`；
步骤边使用 `kind=step`，保留步骤的 `required` 标记，并设置 `blocking=false`。纯文本步骤没有
任务节点，因此不会出现在图边中。`meta` 对象报告活跃状态、节点数、边数、是否截断、数量上限
和查询中的上下文选项。


---

## 7. 评论

### 7.1 列出评论

```http
GET /api/v1/tasks/{task_id}/comments
```

评论以任务 ID 为作用域。列出评论属于只读审计历史，因此归档看板仍可调用；在归档看板上
创建评论会被拒绝。

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": [
    {
      "id": "c_01HX...",
      "board_id": "b_01HX...",
      "task_id": "t_01HX...",
      "author": "alice",
      "author_type": "user",
      "agent_type": null,
      "body": "这里需要确认边界条件。",
      "kind": "note",
      "metadata": {},
      "created_at": 1717520000000
    }
  ]
}
```

### 7.2 添加评论

```http
POST /api/v1/tasks/{task_id}/comments
```

请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "body": "这里需要确认边界条件。",
  "kind": "note",
  "author_type": "user",
  "agent_type": null,
  "author": "alice",
  "metadata": {}
}
```

说明：

- `kind` 默认为 `note`，当前允许 `note|decision|signal`。
- `decision` 记录有实际意义的多选项决策；`body` 始终是可读的后备说明，结构化决策数据放在 `metadata` 中。
- `author_type` 标记评论来源，可取 `user|agent`；省略时服务层默认为 `user`。
- 当 `author_type=agent` 时，`agent_type` 是可选的开放文本，例如 `executor` 或 `reviewer`。
  当 `author_type=user` 时提供非空 `agent_type` 会返回 `400 invalid_input`。
- `metadata` 默认为 `{}`，必须是 JSON 对象；响应同样使用自然 JSON `metadata` 对象。
  普通备注或信号的元数据保持开放且无损，不能因为键名与专用协议碰撞而在事务提交后收紧。
  当 `kind=decision` 时必须包含非空 `options`；每个选项必须有非空的 `slug`、`title` 和
  `detail`；slug 必须是唯一的小写 ASCII slug；`selected` 必须匹配某个选项 slug；
  `reason` 必须非空；`risk` 和 `verification` 如果出现也必须非空。无效的决策元数据返回
  `400 invalid_input`。
- `author` 使用通用的操作者语义；也可以使用 `X-KB-Actor` 或服务端默认操作者。
- 创建评论会写入 `task.comment.created` 事件。


### 7.3 评论端点的精确线上契约

`GET` 与 `POST /api/v1/tasks/{task_id}/comments` 各自拥有独立、闭合的路径与成功响应根；
`POST` 另有独立、闭合的请求根。两者只共享契约拥有的 `ApiComment` 组件和既有共享错误组件。
GET 没有查询或请求体，POST 没有查询；两个端点都已登记并采用精确请求头契约，也已具备
真实路由生产者和契约消费者的精确证据，迁移状态为 `Adopted`。

`ApiComment.author_type` 仅允许 `user|agent`，`kind` 仅允许 `note|decision|signal`，
`agent_type` 是必须出现但可为 `null` 的字段。`metadata` 是开放、无损的响应对象。
创建请求中的 `metadata` 保持为开放 JSON 对象；决策的精确强类型结构由独立的
`metadata.decision.input` / `NoTransport` 契约和真实 CLI 生产者与消费者证据拥有。
运行时原始 JSON 对象继续进入 SQLite 服务层的决策跨字段保护；模式文件不能替代
选中项与选项唯一性、slug、非空值、看板归档以及事务和事件约束。

---

## 8. 执行记录

### 8.1 列出任务执行记录

```http
GET /api/v1/tasks/{task_id}/runs
```

执行记录列表以任务 ID 为作用域，并作为只读审计历史继续对已归档看板开放。

### 8.2 获取执行记录

```http
GET /api/v1/runs/{run_id}
```

### 8.3 获取执行日志

```http
GET /api/v1/runs/{run_id}/log
```

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "run_id": "r_01HX...",
    "content": "执行器输出\n",
    "truncated": false
  }
}
```

说明：

- 响应不包含 `claim_token`。
- 当前最多返回日志末尾 256 KiB；更大的日志会设置 `truncated: true`。
- 若执行记录没有 `log_path` 或文件不存在，返回 `not_found`。
- 若 `log_path` 不在受信任日志目录或文件名不匹配 `<run_id>.log`，返回 `invalid_input`。

### 8.4 读取契约

列表与详情端点分别拥有闭合的路径和成功响应契约，只共享由契约定义的 `ApiRun`。
执行状态是闭合枚举：`running|succeeded|failed|canceled|expired`。
`worker_profile`、`worker_pid`、`finished_at`、`exit_code`、`summary` 和 `error`
都必须出现，但值可以为 `null`。`claim_token` 只出现在显式认领转换的响应中，不进入
执行记录列表或详情；SQLite `log_path` 只供独立日志端点解析受信任文件，也不进入执行记录
列表或详情。上述读取端点均已采用精确契约。

---

## 9. 统计

### 9.1 队列统计

```http
GET /api/v1/stats?board=default
```

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "board_id": "b_01HX...",
    "generated_at": 1717520000000,
    "status_counts": [
      {"status": "ready", "count": 3},
      {"status": "running", "count": 1}
    ],
    "stale_claims": [
      {
        "task_id": "t_01HX...",
        "seq": 12,
        "title": "执行器已失联",
        "claim_owner": "local-worker",
        "claim_expires_at": 1717520000000,
        "last_heartbeat_at": 1717519900000,
        "current_run_id": "r_01HX...",
        "retry_count": 1,
        "max_retries": 3
      }
    ],
    "blocked_reasons": [
      {"reason": "等待操作人员处理", "count": 2}
    ],
    "unplanned_active_tasks": 4,
    "active_parents_with_incomplete_required_steps": 1
  }
}
```

说明：

- `stale_claims` 只包含 `running` 且 `claim_expires_at <= now` 的任务。
- `blocked_reasons` 按数量降序、reason 升序排序。

---

## 10. 事件

### 10.1 列出事件

```http
GET /api/v1/events?board=default&after=0&limit=100
```

`board` 接受看板 slug 或 ID。归档看板的事件仍可读取，便于客户端检查归档后的审计轨迹。

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": [
    {
      "id": 123,
      "event_id": "e_01HX...",
      "board_id": "b_01HX...",
      "task_id": "t_01HX...",
      "run_id": "r_01HX...",
      "kind": "task.claimed",
      "actor": "alice",
      "payload": {"claim_owner":"alice","metadata":{}},
      "created_at": 1717520000000
    }
  ],
  "meta": {
    "next_after": 123
  }
}
```

### 10.2 SSE 事件流

```http
GET /api/v1/stream/events?board=default&after=123
```

SSE 事件：

```text
event: task.claimed
id: 124
data: {"id":124,"event_id":"e_...","board_id":"b_...","task_id":"t_...","run_id":"r_...","kind":"task.claimed","actor":"alice","payload":{"claim_owner":"alice","metadata":{}},"created_at":1717520000000}
```

`board`、`task_id`、`after`、`limit` 是该端点唯一接受的查询键，均只能出现一次；
未知或重复键返回标准 `400 invalid_input` 封装。默认值分别为 `default`、未提供、`0` 和
`100`，运行时会把 `limit` 防御性限制到 `1000`。每个事件严格按 `event`、`id`、`data`
帧顺序输出；`data` 是完整的 `StreamEventData` JSON，不允许额外字段。
`task_id`、`run_id`、`actor` 都是必须存在但可为空：键必须出现，值可以显式为 `null`。
39 个已知事件种类的载荷与种类使用同一个带标签联合；字段缺失、出现额外字段或同级状态
错配时会失败关闭。未来未知事件种类的合法 JSON 载荷保持无损。

重新连接：

- V1 实现会发送当前匹配事件的有限快照后关闭连接；客户端应重新连接，或轮询 `GET /api/v1/events` 获取更新。
- 浏览器客户端可以发送 `Last-Event-ID`，但 V1 只处理 `after` 查询参数。
- V1 有限快照不发送 SSE 注释或心跳帧；因此心跳不是 JSON 载荷契约，`Last-Event-ID`
  也不是已采用的请求头输入契约。这两项只有未来运行时真正实现后才能迁移为强类型契约。
- 若事件已被压缩或清理，客户端应重新获取看板快照。

---

## 11. 看板列与界面设置

### 11.1 列出看板列

```http
GET /api/v1/boards/{board}/columns
```

当前只开放读取接口，服务端没有看板列更新路由，因此暂时不能通过 HTTP API 修改看板列。
返回的列状态仍对应规范任务状态；调用方不得把读取接口推断为可写配置接口。
读取端点的路径、请求头和成功响应精确契约均已采用。

---

## 12. 标签 API

```http
GET /api/v1/boards/{board}/labels
POST /api/v1/boards/{board}/labels
GET /api/v1/boards/{board}/labels/semantics
GET /api/v1/boards/{board}/labels/{label_id}/semantics
PUT /api/v1/boards/{board}/labels/{label_id}/semantics
DELETE /api/v1/boards/{board}/labels/{label_id}/semantics?expected_semantics_hash=<hash>&reason=<text>
GET /api/v1/boards/{board}/labels/atoms
GET /api/v1/boards/{board}/labels/atoms/{atom_ref}/explain
GET /api/v1/boards/{board}/labels/atom-index/status
POST /api/v1/boards/{board}/labels/atom-index/rebuild
GET /api/v1/boards/{board}/labels/atom-index/query?q=<text>&polarity=positive&limit=24
GET /api/v1/tasks/{task_id}/labels
POST /api/v1/tasks/{task_id}/labels
POST /api/v1/tasks/{task_id}/labels/bootstrap
DELETE /api/v1/tasks/{task_id}/labels/{label_id}
GET /api/v1/boards/{board}/signals
GET /api/v1/boards/{board}/signals/review
GET /api/v1/signals/{signal_id}
POST /api/v1/tasks/{task_id}/label-ontology/observations
GET /api/v1/boards/{board}/label-ontology/signals
GET /api/v1/boards/{board}/label-ontology/review
GET /api/v1/label-ontology/signals/{signal_id}
POST /api/v1/boards/{board}/label-ontology/actions
POST /api/v1/boards/{board}/label-ontology/apply/atom
POST /api/v1/boards/{board}/label-ontology/revert
POST /api/v1/boards/{board}/label-ontology/validate
```

看板级标签创建请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "name": "core",
  "color": "blue"
}
```

标签响应结构，用于看板级标签创建和标签列表：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "id": "l_01HX...",
  "board_id": "b_01HX...",
  "name": "core",
  "color": "blue",
  "created_at": 1717520000000,
  "updated_at": 1717520000000
}
```

`POST /api/v1/boards/{board}/labels` 按看板作用域创建标签，并按标签
名称保持幂等。如果该看板上已存在同名标签，响应返回已有标签。空白 `name`
会被拒绝。基础标签标识的增删改查属于词汇表注册表，不属于本体台账；
创建标签标识不会写入 `label_ontology_actions`，也不会创建
`label_semantics` 或 `label_atoms`。

任务标签添加请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "name": "core"
}
```

或批量添加：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "names": ["core", "api"]
}
```

如果需要在绑定时显式创建缺失的标签标识：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "names": ["scratch-label"],
  "create_missing": true
}
```

`POST /api/v1/tasks/{task_id}/labels` 会把 `name` 或 `names` 指定的标签绑定到任务。
`name` 与 `names` 互斥；二者都缺失、二者同时出现或 `names` 为空数组都会返回
`invalid_input`。批量添加在同一事务内执行，并先验证所有标签名称；如果
任一标签为空白或非法，不会创建规范标签，也不会留下部分任务标签绑定。
默认情况下，如果该任务所属看板上还不存在指定名称的标签，请求会返回
`invalid_input`，且不会增加 `labels` 或 `task_labels` 记录。传入
`"create_missing": true` 时，API 只会创建缺失的规范标签标识并绑定到
任务；不会生成 `label_semantics` 或 `label_atoms`。重复绑定已有任务标签关系不会
重复写入。成功响应返回更新后的任务及当前 `labels` 列表；显式创建模式下若
本次创建了标签，响应中的 `meta.created_labels` 会列出新标签。

任务标签引导创建请求：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "name": "database",
  "description": "数据库持久化工作",
  "applies_when": ["涉及 SQLite 迁移"],
  "excludes_when": ["仅调整界面样式"],
  "positive_examples": ["新增数据表迁移"],
  "negative_examples": ["只修改 CSS"],
  "actor": "alice"
}
```

`POST /api/v1/tasks/{task_id}/labels/bootstrap` 是一次性采用新标签的 API：
它会在同一事务内创建任务所属看板上缺失的规范标签，或复用尚无既有语义的同名标签，
写入该标签的 `label_semantics`，同步重建 SQLite `label_atoms`，标记派生的标签原子
向量索引为脏，并把该标签绑定到任务。`name` 按标签名称解析；空白名称会被拒绝。
语义输入会去除两端空白并丢弃空白值，且必须至少
提供 `description` 或一个非空语义数组值。

引导创建 API 默认不会覆盖已有的 `label_semantics`。如果同名标签已经有语义，
请求会失败，并要求调用方改用专用语义变更或提议与采用路径；只有目标标签仍无语义时，
重复调用同一任务和标签才会保持任务标签绑定幂等。成功响应状态为 `201 Created`：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task": {
      "id": "t_01HX...",
      "ref": "default#12",
      "labels": [
        {"id": "l_01HX...", "board_id": "b_01HX...", "name": "database", "color": null}
      ]
    },
    "semantics": {
      "label_id": "l_01HX...",
      "board_id": "b_01HX...",
      "label_name": "database",
      "description": "数据库持久化工作",
      "applies_when": ["涉及 SQLite 迁移"],
      "excludes_when": ["仅调整界面样式"],
      "positive_examples": ["新增数据表迁移"],
      "negative_examples": ["只修改 CSS"],
      "atoms": []
    }
  }
}
```

HTTP 引导创建不包含 CLI `--verify` 的编排：请求体没有向量配置、最低分数或验证标记，
响应也没有 `verification` 字段。该端点不会替调用方重建标签原子向量索引、运行
`label suggest` 或检查分数门槛；需要提交前分阶段验证且失败时零写入的语义时，应使用
CLI `label bootstrap --verify`。API 调用后如需诊断，可显式执行索引重建、建议和审核流程，
但它不具备 CLI 分阶段验证器的同一事务采用契约。

`DELETE /api/v1/tasks/{task_id}/labels/{label_id}` 会移除任务上的指定标签，
`{label_id}` 接受标签 ID 或标签名称。成功响应同样返回更新后的任务及当前 `labels`
列表。只有关联行发生变化时，标签绑定或移除才会写入任务标签事件；该操作不改变任务状态。

### 12.1 标签语义、原子与原子索引

`GET /api/v1/boards/{board}/labels/semantics` 返回当前看板上已定义语义的列表。
`GET /api/v1/boards/{board}/labels/{label_id}/semantics` 返回单个标签的语义；
`{label_id}` 只接受规范的 `l_...` 标签 ID。标签名称允许包含 `/` 等不适合放入路径的字符，
因此语义 API 的路径不支持按标签名称寻址；需要按名称查找时，应先调用
`GET /api/v1/boards/{board}/labels` 获取对应 ID。

`PUT /api/v1/boards/{board}/labels/{label_id}/semantics` 写入已有标签的语义字典，
同步重建该标签的 SQLite `label_atoms`，并标记派生的标签原子向量索引为脏。
请求体：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": "alice",
  "expected_semantics_hash": "optional-current-hash",
  "replace": false,
  "reason": "补充标签审核中反复出现的边界",
  "source_signal_ids": ["los_..."],
  "description": "后端服务工作",
  "applies_when": ["涉及 Rust 服务代码"],
  "excludes_when": ["仅修改 CSS"],
  "positive_examples": ["新增 API 处理器"],
  "negative_examples": ["调整界面间距"],
  "remove_applies_when": [],
  "remove_excludes_when": [],
  "remove_positive_examples": [],
  "remove_negative_examples": []
}
```

默认 `replace=false`，请求按补丁语义处理：`description` 只在提供非空值时覆盖当前描述，
数组字段会追加到对应集合，`remove_*` 数组删除匹配文本；省略字段不会清空已有语义。
传入 `replace=true` 时才完整替换五个语义字段，此时省略的数组视为空数组，并且不能同时传入
任何 `remove_*` 字段。`expected_semantics_hash` 是 CAS 保护条件；如果与当前
`semantics_hash` 不一致，请求返回冲突且不写入。服务会去除两端空白并丢弃空白值。
每次实际改变规范语义或原子的建设性写入，都会在同一 SQLite 事务中写入一条
`update_semantics` 根本体操作，记录操作者、原因、来源信号链接（如有）、前后哈希和一份
变更快照；实际新增或移除的原子通过 `label_ontology_action_atom_effects` 写成
`added` / `removed` 行。仅修改描述的补丁会写一条根操作和零条原子效果；无变化补丁不会写
操作或效果，也不会标记标签原子索引为脏。生成原子时，有描述的标签会生成一个规范
`description` 原子：`label: {name}\ndescription: {description}`；没有描述时才使用
`name` 后备原子。原子文本还会规范化空白：折叠每个非空行内部的空白，但保留规范换行。
同一标签下 `polarity + kind + normalized_text` 相同的原子会去重并保留第一次出现的
`ordinal`；`id` 和 `content_hash` 不包含 `ordinal`，因此仅调整数组顺序不会改变同一文本
原子的标识。响应使用 `DataEnvelope`：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "label_id": "l_01HX...",
    "board_id": "b_01HX...",
    "label_name": "backend",
    "description": "后端服务工作",
    "applies_when": ["涉及 Rust 服务代码"],
    "excludes_when": ["仅修改 CSS"],
    "positive_examples": ["新增 API 处理器"],
    "negative_examples": ["调整界面间距"],
    "created_at": 1717520000000,
    "updated_at": 1717520000000,
    "atoms": [
      {
        "id": "la_...",
        "label_id": "l_01HX...",
        "board_id": "b_01HX...",
        "label_name": "backend",
        "polarity": "positive",
        "kind": "applies_when",
        "text": "涉及 Rust 服务代码",
        "ordinal": 2,
        "content_hash": "...",
        "created_at": 1717520000000,
        "updated_at": 1717520000000
      }
    ]
  }
}
```

`DELETE /api/v1/boards/{board}/labels/{label_id}/semantics?expected_semantics_hash=<hash>&reason=<text>`
是受 CAS 保护的语义清除操作：`expected_semantics_hash` 与非空 `reason` 都必填。
它删除该标签的语义与 SQLite 原子，但不删除规范标签标识或任务标签绑定；同一事务会写入一条
`update_semantics` 根本体操作，变更后快照为空，并为实际移除的原子写入 `removed` 效果，
随后标记标签原子索引为脏。哈希不匹配时，规范数据、操作、效果和脏状态均保持不变。成功返回：

```http
DELETE /api/v1/boards/default/labels/l_01HX/semantics?expected_semantics_hash=sem_abc123&reason=%E5%81%9C%E7%94%A8%E8%BF%87%E6%9C%9F%E8%AF%AD%E4%B9%89
X-KB-Actor: alice
```

<!-- schema-doc: contract=api.label-semantics-delete.response fixture=schemas/fixtures/api/delete-response.v1.valid.json -->
```json
{ "data": { "deleted": true } }
```

`GET /api/v1/boards/{board}/labels/atoms` 返回 SQLite `label_atoms` 的物化投影。
它由 `label_semantics` 和标签名称展开，并随语义变更在同一事务内重建；它是
`lancedb_label_atoms` 派生索引的输入，不能把它描述成独立于语义的第二份语义事实源。

`GET /api/v1/boards/{board}/labels/atoms/{atom_ref}/explain` 按当前原子 ID 或稳定的
`content_hash` 解析原子，并返回 `LabelAtomExplainRecord`：`query`、`atom`、
`current_semantics`、`provenance_actions`、`supporting_signals`、
`validation_history`、`legacy_untracked` 和 `legacy_reason`。当前原子存在但没有
本体来源操作引用其 ID 或内容哈希时，返回 `200` 且 `legacy_untracked=true`；
未知 ID 或哈希返回 `not_found`。

`GET /api/v1/boards/{board}/labels/atom-index/status` 返回标签原子向量索引状态。
服务端的轻量路由通过向量辅助程序适配器报告当前能力。没有向量提供方、适配器不可用或辅助程序
缺失时，仍返回 `200` 和禁用状态。JSON 保留兼容字段 `message`，并额外返回结构化的
`diagnostics: string[]`、`dirty: boolean | null`、`board_dirty: boolean | null`；调用方应使用
结构化字段判断脏状态和错误，不要解析 `message` 文案。相同的 `VectorStoreStatus` 结构也用于
`/api/v1/vector/status`。

`POST /api/v1/boards/{board}/labels/atom-index/rebuild` 通过向量辅助程序适配器调用
标签原子专用的 `rebuild-label-atoms` 命令，重建 `lancedb_label_atoms` 派生索引并更新
`label_atom_index_boards` / `lancedb_label_atoms` 状态。辅助程序或提供方缺失时返回明确的
API 错误，不得写入规范标签事实，也不得把分块存储状态当成标签原子重建成功。
`GET /api/v1/boards/{board}/labels/atom-index/query` 通过向量辅助程序适配器查询派生的
`lancedb_label_atoms` 索引。请求必须提供 `q=<text>` 或 `vector_json=<json-array>` 之一，二者互斥；
`embedding_model` 可选，`include_vector=true` 可要求原始向量命中返回向量，`polarity` 可选且只接受
`positive` / `negative`，`limit` 默认 24。命中项中的 `distance` 是 LanceDB `_distance`，不是
求解器相似度分数。未配置提供方、适配器或辅助程序不可用，或者向量存储不可用时，查询返回明确的
API 错误，且不修改 SQLite 事实。

### 12.2 任务标签建议

```http
GET /api/v1/tasks/{task_id}/labels/suggestions?limit=5&candidate_limit=32&atom_limit=80&max_selected_labels=4&min_score=0.15
```

返回任务级标签建议。若部署中有可用的标签原子向量存储，服务会使用任务标题和描述的嵌入
查询 `lancedb_label_atoms`：正向原子按残差进行多轮检索，负向原子固定使用原始查询检索并
施加惩罚或抑制。求解器在标签组层执行 Group OMP 选择，再把选中标签的高分正向原子向量
作为基底进行非负重新拟合。`coverage` 和 `residual_norm` 来自原子级拟合向量，其中
`coverage = clamp(1 - residual_norm, 0.0, 1.0)`，因此二者不是两份独立证据；
`coverage_cosine` 是原始查询与拟合向量的余弦相似度，可作为独立补充指标。候选标签只有在
试探性重新拟合后带来足够的残差范数降幅，才会进入结果；覆盖率或残差范数达到停止阈值后，
求解器会提前停止，而不是凑满 `max_selected_labels`。候选组与已选标签的语义向量过度相似时
会被跳过，以减少语义重复的标签同时出现在 `selected_labels`；这不会合并或删除规范标签。
`needs_new_label` 是兼容字段，只表示存在需要人工审核的标签覆盖率诊断；具体原因必须读取
`reason_codes`，并结合证据原子、诊断信息和人工语义判断，不能只凭该布尔值创建新词汇。
接口不会创建新标签，也不会写入 `label_semantics` 或 `label_atoms`。

`limit` 只控制响应中 `selected_labels` 和 `candidates` 的最大条数，不会收窄求解器内部
搜索能力。内部能力由 `candidate_limit`、`atom_limit` 和 `max_selected_labels` 分别控制：
候选标签组数量、每轮原子向量检索上限，以及最多进入非负重新拟合的标签数量。所有数量上限都必须是
`1..=1000`；`min_score` 必须在 `0..=1`。

未配置提供方、标签向量适配器或功能不可用、LanceDB 表缺失、索引为空或索引为脏时，
接口仍返回 `200` 和结构化的降级 JSON；普通标签增删改查、任务列表、搜索、筛选和状态转换
不受影响。脏状态判断来自结构化状态和 SQLite 的 `dirty` 字段，不依赖 `message` 文案。
没有提供方时 `needs_new_label=false`，避免误触发自动创建新标签的流程。

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task_id": "t_01HX...",
    "board_id": "b_01HX...",
    "selected_labels": [
      {
        "label_id": "l_01HX...",
        "label_name": "backend",
        "score": 0.82,
        "weight": 0.82,
        "already_applied": false,
        "evidence_atoms": [
          {
            "atom_id": "la_...",
            "label_id": "l_01HX...",
            "label_name": "backend",
            "polarity": "positive",
            "kind": "applies_when",
            "text": "涉及服务端代码",
            "score": 0.91
          }
        ],
        "negative_evidence_atoms": []
      }
    ],
    "candidates": [],
    "coverage": 0.82,
    "coverage_cosine": 0.91,
    "residual_norm": 0.18,
    "needs_new_label": false,
    "reason_codes": ["degraded_result", "label_atom_index_dirty"],
    "degraded": true,
    "diagnostics": ["label_atom_index_dirty"]
  }
}
```

稳定的 `diagnostics` 包括：

- `vector_store_disabled`
- `label_atom_index_dirty`
- `label_atom_index_empty`
- `label_atom_index_error`
- `vector_query_error`

非降级覆盖率审核的稳定 `reason_codes` 包括：

- `no_selected_labels`
- `coverage_below_threshold`
- `residual_above_threshold`
- `unexplained_residual`

### 12.3 标签语义提议

```http
POST /api/v1/tasks/{task_id}/label-proposals?limit=5&candidate_limit=32&atom_limit=80&max_selected_labels=4&min_score=0.15
GET /api/v1/tasks/{task_id}/label-proposals
GET /api/v1/label-proposals/{proposal_id}
POST /api/v1/label-proposals/{proposal_id}/accept
POST /api/v1/label-proposals/{proposal_id}/reject
```

`POST /api/v1/tasks/{task_id}/label-proposals` 创建一次新的标签提议尝试。
请求体可为空或仅包含 `actor`；此时默认提供方不可用，接口返回 `200`
和降级的尝试结果，不创建规范标签、`label_semantics`、`label_atoms` 或
`task_labels`。

提供方边界：API 当前只支持空的默认提供方，或请求体中显式传入的本地离线候选。
真实 LLM 提供方不在 `kanban-sqlite` 中实现；如果未来服务端支持本机 AI 运行时，
必须在服务端、本地层或独立 AI crate 层实现 `LabelProposalProvider` 适配器，
并把候选交给 SQLite 服务层做确定性校验和持久化。

带本地离线提供方输出时：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "proposal": {
    "name": "database",
    "description": "数据库持久化工作",
    "applies_when": ["涉及 SQLite 迁移"],
    "excludes_when": ["仅调整界面样式"],
    "positive_examples": ["新增数据表迁移"],
    "negative_examples": ["只修改 CSS"]
  },
  "actor": "alice",
  "source_signal_ids": ["los_..."],
  "ontology_actor": {"name": "codex", "type": "agent", "agent_type": "codex"},
  "allow_retarget": false,
  "retarget_reason": null
}
```

数组字段省略时按空数组处理。服务先读取当前标签建议的启发式 `coverage`、
`coverage_cosine`、`residual_norm` 和现有标签第一名。覆盖率充足时不写提议；
覆盖率不足、候选语义有效，且残差第一名加间隔校验明确通过时，返回 `201` 并持久化
状态为 `proposed` 的提议。候选与现有标签发生规范化名称冲突时，会以 `rejected` 状态
持久化，`diagnostics` 包含 `near_duplicate_label_conflict`。规范化名称冲突忽略大小写、
空白和标点，是确定性的近似重复启发式。

`source_signal_ids` 可选；传入时，提议创建成功后会在同一事务中写入
`create_label_proposal` 本体操作，并通过操作与信号的链接记录哪些已确认的词汇缺口信号
支持该提议。提议行与来源操作要么同时写入，要么一起回滚。来源信号默认必须属于同一看板、
状态为 `confirmed`、种类为 `vocabulary_gap`、`proposed_action` 为 `bootstrap_label`，
且规范化后的 `proposed_label_name` 等于提议名称。`ontology_actor` 只控制
`create_label_proposal` 操作的来源；省略时使用 `actor` 字符串作为 `type=user` 的操作者。
确需重定向同一看板上已确认的来源信号时，必须传入 `allow_retarget=true` 和非空
`retarget_reason`；原因和来源信号原始的目标或提议标签会写入
`change_json.retarget_override`。重定向不会放宽看板和状态要求。

POST 提议路由接受与标签建议相同的查询参数。`limit` 只截断建议输出；
`candidate_limit`、`atom_limit`、`max_selected_labels` 和 `min_score` 用于调节底层求解器的
启发式覆盖率和残差校验。

服务端配置了可用的向量提供方时，提议尝试与标签建议使用同一套 LanceDB 标签原子存储。
覆盖率不足的候选会在持久化前执行残差第一名加间隔校验：候选语义的残差分数和现有标签
第一名都根据返回的原子向量在本地计算余弦相似度，不从 LanceDB 距离推导；候选必须超过
现有标签第一名，且差值达到固定间隔。校验失败时，候选仍会以 `rejected` 提议持久化，
`diagnostics`
包含 `label_proposal_residual_top1_failed` 或
`label_proposal_residual_margin_insufficient`。未配置提供方、功能不可用或向量检索失败时
返回降级尝试，不创建规范标签、`label_semantics`、`label_atoms` 或 `task_labels`。
如果残差校验不可用或已降级，且没有明确通过第一名加间隔校验，本次尝试返回
`proposal=null`，不新增提议行；`diagnostics` 包含
`label_proposal_residual_validation_unavailable` 和具体原因。

尝试响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task_id": "t_...",
    "board_id": "b_...",
    "proposal": null,
    "degraded": true,
    "diagnostics": ["label_proposal_provider_unavailable", "vector_store_disabled"],
    "heuristic_coverage": 0.0,
    "heuristic_coverage_cosine": 0.0,
    "heuristic_residual_norm": 1.0,
    "top1_existing_label_id": null,
    "top1_existing_label_name": null
  }
}
```

接受或拒绝的请求体：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "reason": "覆盖率不足，接受新标签",
  "actor": "alice",
  "source_signal_ids": ["los_..."],
  "ontology_actor": {"name": "codex", "type": "agent", "agent_type": "codex"},
  "allow_retarget": false,
  "retarget_reason": null
}
```

接受操作只允许状态为 `proposed` 的提议。成功后会通过与任务标签引导创建相同的采用原语，
创建规范 `labels` 行以及对应的 `label_semantics` 和 `label_atoms`，标记标签原子索引为脏，
并在同一事务中写入一条 `bootstrap_label` 根本体操作和对应的新增原子效果。提议状态、
规范写入和来源操作要么一起成功，要么一起回滚。它不会自动写入 `task_labels`。
`source_signal_ids` 可选；省略时仍会记录引导创建操作，但没有操作与信号的链接。传入时，
接受操作会通过这些链接记录新标签的引导创建来源。来源信号必须属于同一看板且处于
`confirmed`。`actor` 字符串仍用于提议决策事件；`ontology_actor` 只控制接受操作产生的
`bootstrap_label` 本体操作来源。省略 `ontology_actor` 时，引导创建操作使用 `actor`
字符串作为 `type=user` 的操作者。`type=agent` 必须提供非空 `agent_type`；
`type=user` 不能提供 `agent_type`。来源信号默认还必须是
`vocabulary_gap` 加 `bootstrap_label`，且规范化后的 `proposed_label_name` 必须等于提议名称。
确需重定向同一看板上已确认的来源信号时，必须传入 `allow_retarget=true` 和非空
`retarget_reason`；引导创建操作中的 `change_json.retarget_override` 会记录原因、来源信号
原始目标或提议标签，以及最终提议或结果标签。如果提议已有 `create_label_proposal` 操作，
接受操作产生的 `bootstrap_label` 操作会把 `parent_action_id` 指向该创建操作。重定向不会
放宽看板和状态要求。拒绝操作把提议标记为 `rejected`，不接受 `source_signal_ids`、
`ontology_actor` 或重定向选项。对已接受或已拒绝的提议再次决策，会返回标准
`400 invalid_input` 错误封装。

### 12.4 通用信号台账

通用信号台账 API 提供按看板划分的只读收件箱，用于展示代理或产品在看板工作流中记录的
通用信号，例如 CLI 参数摩擦、提示误导、参数设计问题或操作人员发现。它独立于标签本体台账；
这些端点不会创建、确认、拒绝、解决或取代信号，也不会把通用信号混入本体审核分组。

```http
GET /api/v1/boards/{board}/signals?status=open&kind=agent_cli_friction&task_ref=default%23123&include_all=false&limit=100
GET /api/v1/boards/{board}/signals/review?status=confirmed&kind=agent_cli_friction&task_ref=default%23123&include_all=false&limit=100
GET /api/v1/signals/{signal_id}
```

`GET /api/v1/boards/{board}/signals` 和 `/signals/review` 返回同一只读 DTO；
`review` 端点是桌面端或操作人员控制台的语义化入口。默认只返回 `open`
和 `confirmed`。可重复传 `status` 和 `kind`，并按 `task_ref` 过滤。
`include_all=true` 且没有显式 `status` 时返回完整历史；`limit` 使用普通列表上限。
这些列表和审核路由以看板为作用域，只返回该看板的信号行。
`GET /api/v1/signals/{signal_id}` 是操作人员全局详情查询，用于从反向链接、收件箱行或审计
记录直接打开已知信号。该详情路由不会改变信号的 `board_id` 事实，也不会让按看板划分的
列表或审核接口泄漏其他看板的信号。

`signal_observations.task_id`、`run_id` 和 `comment_id` 是来源与历史的软引用。
当前服务写入路径、诊断命令和导入最终门禁会维护这些引用与观察记录所属看板的一致性；
未来若需要硬化所有来源关系，可迁移为按看板组合的外键。

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": [
    {
      "id": "sig_...",
      "board_id": "b_...",
      "observation_id": "obs_...",
      "kind": "agent_cli_friction",
      "title": "--require 参数命名不符合 agent 惯用预期",
      "summary": "代理尝试使用 --required/--requires，实际 CLI 只接受 --require。",
      "severity": "medium",
      "status": "open",
      "dedupe_key": "kanban-task-create-require",
      "superseded_by_signal_id": null,
      "reviewed_by": null,
      "reviewed_at": null,
      "review_reason": null,
      "created_at": 1782930000000,
      "updated_at": 1782930000000,
      "observation": {
        "id": "obs_...",
        "board_id": "b_...",
        "task_id": "t_...",
        "task_ref_snapshot": "default#123",
        "run_id": "r_...",
        "comment_id": null,
        "actor": "local-agent",
        "agent_type": "automation",
        "source": "cli-hook",
        "evidence": {"command":"kanban task create --required ..."},
        "created_at": 1782930000000
      }
    }
  ]
}
```

`GET /api/v1/signals/{signal_id}` 返回单条 signal：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "id": "sig_...",
    "observation": {}
  }
}
```

### 12.5 标签本体台账

标签本体台账 API 记录任务标注过程、审核队列、本体变更来源和验证历史。台账不会自动修改
任务标签；规范绑定仍通过任务标签 API 或 CLI 完成。

所有本体操作者对象使用 `{ "name": string, "type": "user"|"agent",
"agent_type": string|null }`。`type=agent` 必须提供非空 `agent_type`；
`type=user` 必须省略或传 `null`。

```http
POST /api/v1/tasks/{task_id}/label-ontology/observations
GET /api/v1/boards/{board}/label-ontology/signals?status=open&kind=false_negative&task_ref=default%2312&target_label_ref=cli&proposed_label_name=database&include_all=false&limit=100
GET /api/v1/boards/{board}/label-ontology/review?group_by=label&include_all=false&limit=100
GET /api/v1/label-ontology/signals/{signal_id}
POST /api/v1/boards/{board}/label-ontology/actions
POST /api/v1/boards/{board}/label-ontology/apply/atom
POST /api/v1/boards/{board}/label-ontology/revert
POST /api/v1/boards/{board}/label-ontology/validate
```

`POST /api/v1/tasks/{task_id}/label-ontology/observations` 在一个事务中写入观察记录和
子信号。HTTP 端点不会自行运行 `label suggest`；调用方必须传入由工具采集且未改写的
`suggestion_snapshot`，或在没有建议证据时显式传入空快照。服务端会从快照派生观察指标，
代理或审核者只提交候选、最终判断、信号、候选原子和理由。请求体：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": {"name": "label-agent", "type": "agent", "agent_type": "local"},
  "agent_candidates": [],
  "suggestion_snapshot": {
    "selected_labels": [],
    "coverage": 0.61,
    "coverage_cosine": 0.74,
    "residual_norm": 0.39,
    "needs_new_label": false,
    "degraded": false,
    "diagnostics": []
  },
  "final_decision": {},
  "capture_fingerprint": "optional-stable-key",
  "signals": [
    {
      "kind": "false_negative",
      "target_label_ref": "cli",
      "related_labels": [],
      "proposed_action": "add_positive_atom",
      "candidate_atom": {
        "polarity": "positive",
        "kind": "applies_when",
      "text": "扩展 CLI 子命令、命令参数、帮助输出或机器可读的 JSON 行为"
      },
      "proposal": {},
      "agent_selected": true,
      "suggest_state": "candidate",
      "suggest_score": 0.08,
      "suggest_rank": 6,
      "final_selected": true,
      "rationale": "该任务扩展了 CLI 接口。",
      "confidence": 0.9
    }
  ]
}
```

面向新客户端的 HTTP 本体 DTO 使用自然 JSON 字段：`agent_candidates`、
`suggestion_snapshot`、`final_decision`、`diagnostics`，信号中的 `related_labels` 和
`proposal`，操作中的 `change` 和 `validation`，以及验证请求中的 `validation`。
公开 HTTP API 不再接受旧的转义字符串同级请求字段，例如 `related_labels_json`、
`proposal_json`、`change_json`、`validation_json` 和观察记录中的 `*_json` 别名。
出现未知旧字段时会以 `400 invalid_input` 失败关闭；客户端必须发送自然 JSON 字段。
当 `suggestion_snapshot` 包含 `coverage`、`coverage_cosine`、`residual_norm`、
`needs_new_label`、`degraded` 或 `diagnostics` 时，服务端会从快照派生持久化的观察指标。
如果请求同时提供对应的顶层 `suggest_*` 字段或 `diagnostics`，且值发生冲突，则返回
`400 invalid_input`。新客户端不应在顶层重复快照事实。

服务会读取当前任务快照、解析 `target_label_ref`，并计算规范化的提议标签名称、信号键和
候选原子内容哈希。`capture_fingerprint` 为空时会根据任务、快照和信号派生；同一看板上的
重复指纹会被唯一约束拒绝。观察响应返回新建观察记录并展开子 `signals`。观察记录包含用于
完整审计的 `task_snapshot_json.content_hash`，以及只基于标签建议输入
（规范化标题加描述）的 `suggest_input_hash`；后者用于后续验证的可比性判断。

信号输入会在写入前接受本体契约校验。`candidate_atom` 中，`applies_when` 和
`positive_example` 只能使用 `positive` 极性，`excludes_when` 和 `negative_example`
只能使用 `negative` 极性。`add_positive_atom` 必须提供目标标签和正向候选原子；
`add_negative_atom` 必须提供目标标签和负向候选原子；`update_semantics` 必须提供目标标签；
`bootstrap_label` 必须提供 `proposed_label_name`；`rename_label` 必须提供目标标签和
`proposed_label_name`；`split_label` 和 `merge_labels` 必须提供目标标签及非空的
`related_labels`。观察指标 `suggest_coverage`、`suggest_coverage_cosine`、
`suggest_residual_norm` 以及信号指标 `suggest_score` 和 `confidence` 必须是有限的
`0.0..=1.0`；`suggest_rank` 必须为 `null` 或 `>= 1`。违反这些契约的请求返回
`400 invalid_input`，不会写入观察记录或信号。`rename_label`、`split_label` 和
`merge_labels` 当前只作为审核信号的提议操作保存，不能通过公开 HTTP 路由写入规范结构
变更操作；旧的结构计划行只读展示为不受支持的验证要求。

`GET /api/v1/boards/{board}/label-ontology/signals` 默认只返回 `open` 和
`confirmed`。可重复传 `status` 和 `kind`，并按 `task_ref`、`target_label_ref`、
`proposed_label_name`、`include_all`、`limit` 过滤。

`GET /api/v1/boards/{board}/label-ontology/review` 返回只读聚合审核队列。
`group_by` 支持 `label`、`candidate_atom`、`proposed_label`，以及需要显式启用的 `cluster`，
默认 `label`；`include_all=false` 默认只聚合 `open` 和
`confirmed` 信号，`true` 时包含完整历史；`limit` 限制分组数量。响应
`meta` 回显 `group_by`、`include_all` 和 `limit`。每个分组包含：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "group_by": "label",
  "key": "lab_...",
  "label_id": "lab_...",
  "label_name": "cli",
  "candidate_atom_polarity": "positive",
  "candidate_atom_kind": "applies_when",
  "candidate_text": "扩展 CLI 子命令",
  "candidate_content_hash": "14ada47e4b0566c5",
  "proposed_label_name": null,
  "proposed_label_name_normalized": null,
  "cluster_key": null,
  "cluster_reason": null,
  "task_count": 2,
  "signal_count": 3,
  "open_count": 2,
  "confirmed_count": 1,
  "resolved_count": 0,
  "rejected_count": 0,
  "superseded_count": 0,
  "degraded_count": 1,
  "average_score": 0.31,
  "median_score": 0.28,
  "oldest_signal_at": 1781780000000,
  "latest_signal_at": 1781780100000,
  "sample_task_refs": ["default#12"],
  "signal_ids": ["los_..."],
  "action_count": 1,
  "action_ids": ["loa_..."],
  "proposal_ids": [],
  "labels": [{"id": "lab_...", "name": "cli"}],
  "candidate_atom_variants": [
    {
      "content_hash": "14ada47e4b0566c5",
      "polarity": "positive",
      "kind": "applies_when",
      "text": "扩展 CLI 子命令",
      "signal_count": 2
    }
  ]
}
```

分组依次按去重后的 `task_count` 降序、`confirmed_count` 降序、
`latest_signal_at` 降序和 `key` 升序排列。`group_by=cluster` 是可禁用的只读辅助视图：
默认不会启用，不写规范原子，不确认、应用、验证或关闭信号，也不会创建新的 SQLite 事实表。
聚类键会在每次请求时根据已有信号文本重建：优先使用词法规范化后的候选文本，其次使用提议
标签，再其次使用理由，最后退回到种类、操作、目标和提议标签作用域的组合。所有聚类键都带有
信号种类、提议操作、目标标签和提议标签作用域，避免跨标签、操作或边界误合并；
`cluster_reason` 说明键的来源。`GET /api/v1/label-ontology/signals/{signal_id}`
返回：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "signal": {},
    "observation": {},
    "actions": []
  }
}
```

`POST /api/v1/boards/{board}/label-ontology/actions` 写入审核或生命周期操作：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": {"name": "alice", "type": "user", "agent_type": null},
  "action_type": "confirm",
  "signal_ids": ["los_..."],
  "reason": "在多个独立 CLI 任务中观察到",
  "superseded_by_signal_id": null,
  "parent_action_id": null,
  "target_label_ref": null,
  "result_label_ref": null,
  "result_atom_id": null,
  "result_atom_content_hash": null,
  "result_proposal_id": null,
  "canonical_before_hash": null,
  "canonical_after_hash": null,
  "validation_requirement": null,
  "validation_status": null,
  "validation_effective_outcome": null
}
```

该公共操作端点只接受生命周期操作类型：`confirm`、`reject`、`supersede` 和
`resolve_no_change`，并会同步更新来源信号状态。请求中的
`parent_action_id`、`target_label_ref`、结果字段、规范哈希、`change`、
`validation_requirement`、`validation_status`、
`validation_effective_outcome` 和 `validation` 必须为
`null` 或省略；否则返回 `invalid_input`。`add_positive_atom`、`add_negative_atom`、
`adopt_existing_atom`、`update_semantics`、`create_label_proposal`、`bootstrap_label`、
`revert_ontology_mutation`、`validate` 等变更或验证操作类型，不允许通过该通用端点写入；
规范变更的来源必须由语义 PUT、应用原子、创建或接受提议、任务标签引导创建或验证等专用
路由在同一事务内写入。写入 `supersede` 时会沿替代关系的 `superseded_by_signal_id`
链检查；若链路回到任一来源信号，或替代链本身已有环，则返回 `invalid_input`，不会写入新的
取代操作。

`POST /api/v1/boards/{board}/label-ontology/apply/atom` 对已有标签执行
读取、修改并更新语义，并写入原子来源操作：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": {"name": "codex", "type": "agent", "agent_type": "codex"},
  "signal_ids": ["los_1", "los_2"],
  "label_ref": "cli",
  "kind": "applies_when",
  "text": "扩展 CLI 子命令、命令参数、帮助输出或机器可读的 JSON 行为",
  "reason": "多个 CLI 接口任务反复出现假阴性信号",
  "allow_retarget": false,
  "retarget_reason": null
}
```

来源信号必须属于同一看板且已 `confirmed`。`kind` 只接受
`applies_when`、`positive_example`、`excludes_when`、`negative_example`。如果规范内容
实际新增了原子，成功后返回 `add_positive_atom` 或 `add_negative_atom` 操作，记录结果
原子的软引用、内容哈希、变更前后规范哈希、一份变更快照和一个 `added` 原子效果，并把
`validation_requirement` 设为 `required`。如果相同内容的原子已经存在，成功后返回
仅记录来源的 `adopt_existing_atom` 操作，记录现有原子的软引用、相同的前后规范哈希和
来源信号链接；该操作不修改语义或原子，不标记原子索引为脏，
`validation_requirement=none`，有效结果为 `not_required`。默认要求所有带
`target_label_id` 的来源信号都指向 `label_ref`；不匹配时返回 `400 invalid_input`
并列出违规信号 ID。审核者可以泛化原子文本，不要求它等于来源信号中的候选文本。
确需重定向同一看板上已确认的信号时，必须传入 `allow_retarget=true` 和非空
`retarget_reason`；操作中的 `change_json.retarget_override` 会记录原因、来源信号原始
目标或提议标签，以及最终目标标签。重定向不会放宽看板和状态要求。只有实际新增规范原子时，
该路由才会标记标签原子索引为脏；向量重建和建议验证在事务外执行。

`POST /api/v1/boards/{board}/label-ontology/revert` 追加可追溯的回滚操作，并把目标标签语义
恢复为被撤销变更操作的变更前快照：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": {"name": "reviewer", "type": "user", "agent_type": null},
  "target_action_id": "loa_...",
  "expected_current_hash": "optional-current-semantics-hash",
  "reason": "回滚仅用于测试的原子变更"
}
```

当前只支持 `add_positive_atom`、`add_negative_atom` 和 `update_semantics`。路由要求当前
规范语义哈希仍等于 `target_action_id` 的 `canonical_after_hash`；
`expected_current_hash` 非空时还必须等于当前哈希。成功后返回
`revert_ontology_mutation` 操作：`parent_action_id` 指向被撤销操作，来源信号链接从目标
操作复制，`change` 记录被撤销操作、回滚前后快照和 `index_dirty=true`，并为本次回滚实际
新增或移除的原子写入原子效果，随后标记标签原子索引为脏。该操作的
`validation_requirement` 为 `unsupported`，可以记录外部失败或部分诊断，但不会被当作可由
可信验证通过的待验证项。该路由不会删除或修改原操作，也不处理引导创建的标签标识或任务
绑定回滚；CLI 分阶段引导验证的失败路径会在提交前保持零写入，不再依赖提交后的恢复流程。

`POST /api/v1/boards/{board}/label-ontology/validate` 追加外部证明验证操作。HTTP 路由接收
调用方提交的自然 JSON `validation`，但当前不会运行向量重建、索引查询或 `label suggest`，
因此不能产生可信自动化的 `passed`。需要可信自动化验证时，应使用 CLI
`label ontology validate --trusted`，由工具采集索引和建议证据后写入。

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": {"name": "codex", "type": "agent", "agent_type": "codex"},
  "parent_action_id": "loa_...",
  "signal_ids": ["los_1", "los_2"],
  "reason": "重建原子后，来源任务仍未选中目标标签",
  "validation_status": "failed",
  "validation": {
    "evidence_type": "external_attestation",
    "reviewer": "codex",
    "cases": [
      {
        "signal_id": "los_1",
        "case_type": "positive_atom",
        "passed": false,
        "before": {
          "target": {"label_id": "l_cli", "selected": false, "score": 0.12},
          "coverage": 0.61
        },
        "after": {
          "degraded": false,
          "target": {"label_id": "l_cli", "selected": false, "score": 0.14},
          "coverage": 0.60,
          "notes": "人工审核持久化的建议输出后，结果未达到通过标准"
        }
      }
    ]
  }
}
```

服务会把调用方提供的 `validation` 包入验证封装，并附上来源信号案例、观察时任务快照或
建议输入哈希与当前任务哈希的对比、父操作结果引用和摘要。公开的提供或采集载荷只保存在
顶层 `manual`；生成的 `cases[]` 通过 `after.manual_case_ref` 指向
`manual.cases[]` 中对应信号的原始证据，避免多信号验证把同一载荷重复存入每个案例。
`parent_action_id` 必须指向同一看板上 `validation_requirement=required` 的规范变更操作，
且父操作必须带有规范结果证据，例如原子、结果标签或提议引用、规范哈希和非空变更快照。
HTTP 提供的 JSON 属于外部证明；它可以保存 `failed` 或 `partial` 诊断，但
`validation_status="passed"` 会返回 `invalid_input`，因为验证通过需要工具采集的
`trusted_automated` 证据。`unsupported` 的父操作可以记录外部失败或部分诊断，但不能通过。
结构化字段或字符串 `"automated"` 本身不构成可信来源。

可信自动化验证的持久化载荷由 CLI 采集器生成，而不是由 HTTP 调用方手写：顶层包含
`evidence_type="trusted_automated"`、`collector.source`、非空 `embedding_model`、
对象 `solver_options`、干净的 `index.status`、`index.generation`，以及覆盖每个已链接来源
信号的 `cases[]`。CLI 采集器在较长的 SQLite 事务之外重建原子索引并运行建议；写入操作时，
服务会在短事务中重新核验父操作、来源信号、变更后规范哈希、索引脏或错误状态和代次。
“可信”表示证据由工具采集、当前哈希与索引代次一致，并在指定案例和对照上机械通过；
它不是全局语义正确性的证明。

强类型策略按父操作检查：

- `add_positive_atom`：`case_type="positive_atom"`，`after.degraded=false`；
  `after.evidence_atoms[]` 必须包含父操作的 `result_atom_id` 或
  `result_atom_content_hash`；目标标签必须已选中或分数不低于 0.50；
  分数和覆盖率不能比变更前恶化。
- `add_negative_atom`：`case_type="negative_atom"`，`after.evidence_atoms[]`
  不用于结果负向原子校验；父操作的结果原子必须出现在
  `after.negative_evidence_atoms[]`。假阳性任务必须证明
  `after.target.selected=false`，或变更前后分数都存在且变更后分数低于变更前分数。
  必须提供至少一个 `after.positive_controls[]`，且每个对照都已通过且未退化；
  若没有正向对照，必须提供带非空原因的
  `after.positive_control_waiver`。
- `bootstrap_label`：`case_type="bootstrap_label"`，所有已链接来源信号都必须有通过的
  案例；新标签或结果标签必须已选中或分数不低于 0.50；证据原子必须来自结果标签。

验证可比性默认使用观察记录的 `suggest_input_hash`。状态、`updated_at`、`lock_version`
或任务标签绑定只改变完整快照时，会写入 `task_metadata_drift` 或
`label_binding_drift` 警告，不会让已通过的验证过期。标题或描述变化会写入
`suggest_input_drift` 并使案例不可比较；旧观察记录缺少 `suggest_input_hash` 时写入
`legacy_suggest_input_hash_missing`，不能静默通过。`passed` 会把已链接来源信号转为
`resolved`；`failed` 和 `partial` 会保留信号，供后续修正或人工处理。

---

## 13. 搜索

### 13.1 搜索任务

```http
GET /api/v1/search/tasks?board=default&q=needle&status=ready&label=backend&assignee=worker-a&include_archived=false&limit=20&offset=0
```

默认的 CLI 和服务端构建启用 `tantivy-backend`。SQLite 数据库旁存在 `index/v1/tasks/` 时，
搜索使用 Tantivy 任务索引。Tantivy 索引缺失、损坏或过期，或者二进制显式使用
`--no-default-features` 构建时，会回落到 SQLite，并附带过期元数据。搜索会匹配任务标题、
描述、评论、执行摘要或错误，以及事件种类和载荷。

`label` 按标签名称或 ID 过滤，可重复，并在评分和分页前使用 AND 语义。
带标签过滤的搜索即使存在可用的 Tantivy 索引，也会使用 SQLite 后备路径，
以确保结果反映当前任务标签关联行。

任务引用形状的 `q` 始终使用 SQLite 精确匹配语义，即使当前存在可用的 Tantivy 索引：
纯数字 `12` 和 `#12` 匹配请求看板内的序号；`board#12` 和 `board/#12`
只在显式看板等于请求看板时匹配；`t_...` 只匹配请求看板内的任务 ID。
任务引用形状的查询不会从标题、描述、评论、执行记录或事件中返回模糊匹配。

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "hits": [
      {
        "task_id": "t_01HX...",
        "seq": 12,
        "score": 60.0,
        "snippet": "就绪任务的规格命中内容",
        "task": {
          "id": "t_01HX...",
          "seq": 12,
          "status": "ready",
          "title": "实现状态机"
        }
      }
    ],
    "meta": {
      "backend": "sqlite",
      "stale": false,
      "index_version": null,
      "last_event_id": 42,
      "index_lag_events": 0
    }
  },
  "meta": {
    "limit": 20,
    "offset": 0
  }
}
```

任务变更不会在 SQLite 事务内写入 Tantivy。使用 `tantivy-backend` 运行 `kanban serve` 时，
后台循环会在启动后立即尝试一次 `sync_search_index`，随后默认每
`--search-sync-interval-ms` 毫秒同步一次（默认 `5000`；`0` 表示禁用）。普通任务变更后仍可
手动运行 `kanban index sync`；`kanban index rebuild` 会替换派生索引。Tantivy 状态按看板
存放在 `app_settings` 的 `search.tasks.state.<board_id>` 下，并可随现有导出与导入往返。

### 13.2 按状态搜索任务窗口

```http
GET /api/v1/search/tasks/by-status?board=default&q=needle&status=ready&status=review&include_archived=false&limit=50&offset=0
```

这个只读端点把看板上的多列搜索合并为一个请求。它接受与
`GET /api/v1/search/tasks` 相同的查询文本、看板、标签、执行者、归档和分页参数，
但会为每个重复的 `status` 返回独立搜索窗口。`limit` 和 `offset` 分别作用于每个状态窗口。
响应中的状态顺序与查询参数顺序一致；省略 `status` 时返回空的 `statuses` 数组。

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "statuses": [
      {
        "status": "ready",
        "tasks": [
          {
            "id": "t_01HX...",
            "ref": "default#12",
            "status": "ready",
            "title": "实现状态机"
          }
        ],
        "search_meta": {
          "backend": "sqlite",
          "stale": false,
          "index_version": null,
          "last_event_id": 42,
          "index_lag_events": 0
        },
        "page": {
          "limit": 50,
          "offset": 0,
          "total": null
        }
      }
    ]
  },
  "meta": {
    "limit": 50,
    "offset": 0
  }
}
```

### 13.3 搜索索引状态

```http
GET /api/v1/search/status?board=default
```

响应：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "backend": "sqlite",
    "derived_index": false,
    "stale": false,
    "index_version": null,
    "last_event_id": 42,
    "index_lag_events": 0,
    "message": "SQLite fallback search is active; no derived index exists yet"
  }
}
```

当前的 `MAX(task_events.id)` 大于 Tantivy 持久化的 `last_event_id` 时，
`stale=true`，`index_lag_events` 会报告高水位差值。后台同步被禁用、延迟或失败时，
搜索仍会返回当前 SQLite 后备结果和过期元数据，而不会信任已经落后的派生索引。

---

## 14. 图与向量派生能力

本节三个只读端点均已采用精确查询、请求头和成功响应契约，迁移状态均为 `Adopted`。
SQLite 仍是事实源；图和向量后端是可重建的派生能力。

### 14.1 图后端状态

```http
GET /api/v1/graph/status?board=default
```

`board` 可选，默认为 `default`。响应的 `data` 包含 `backend`、`enabled` 和
供人阅读的 `message`。辅助程序缺失等可降级状态仍以 `200` 返回，并设置
`enabled=false`。

### 14.2 查询图邻居

```http
GET /api/v1/graph/neighbors?board=default&entity_uri=kb%3A%2F%2Ftask%2Ft_example&predicate=depends_on&limit=50
```

| 参数 | 是否必填 | 默认值 | 说明 |
|---|---|---:|---|
| `board` | 否 | `default` | 看板 slug 或 ID。 |
| `entity_uri` | 是 | 无 | 要查询的实体 URI。 |
| `predicate` | 否 | 无 | 可选的关系谓词过滤，取值见下文。 |
| `limit` | 否 | `50` | 返回关系数量上限，范围 `0..=1000`。 |

`predicate` 只接受 `belongs_to_board`、`belongs_to_task`、`depends_on`、`produced_by`、
`generated_by`、`references_artifact`、`related_to`、`uses_skill`、`uses_context`、
`derived_from`、`supersedes`、`similar_to`、`requires_review` 或 `waiting_for_user`。

响应以 `data` 数组返回关系记录，以 `meta.limit` 回显实际数量上限。每条关系包含
`subject_uri`、`predicate`、`object_uri`、`graph_uri`、来源信息、开放的 `metadata`，
以及创建和更新时间。

### 14.3 向量后端状态

```http
GET /api/v1/vector/status?board=default
```

`board` 可选，默认为 `default`。响应的 `data` 包含 `backend`、`enabled`、`message`、
`diagnostics`、必需但可为 `null` 的 `dirty` 和 `board_dirty`，以及有值时才出现的
`generation`。辅助程序缺失或输出不可用时会通过结构化状态说明降级。

---

## 15. 维护

### 15.1 诊断

```http
POST /api/v1/maintenance/doctor
```

响应包含 SQLite 完整性、迁移或用户版本、已过期的运行中任务、孤立执行记录检查、依赖环数量、
已归档依赖边数量、缺失或可疑执行日志数量、依赖、规格和排期违规的可执行状态不变量统计、
基础关系一致性诊断、标签本体台账诊断和知识底座诊断。已归档父任务指向活跃子任务的边属于
允许保留的历史依赖边；活跃父任务指向已归档子任务的边会被计数。

基础关系诊断是只读的：

- `consistency_errors` 和 `consistency_warnings` 汇总基础关系行的看板一致性发现。
- `consistency_issues[]` 以 `severity`、`code`、`message` 和 `record_ids` 报告结构化问题。
- 覆盖的表包括 `task_labels`、`task_dependencies`、`task_steps`、`task_execution_plans`、
  `task_runs`、`task_comments`、`signal_observations`、`signals`、`task_events` 和
  `task_attachments`。
- v24 及以上数据库要求通用信号台账具备 `signal_observations` 和 `signals`。
- 硬错误表示某行的 `board_id` 与它引用的任务、标签、执行记录、评论或观察记录所属看板不同。
  消息包含 `table`、`row`、`row_board`、`referenced` 和 `referenced_board`。
- v25 及以上数据库为 `signals.observation_id` 和 `signals.superseded_by_signal_id`
  添加按看板组合的外键。
- 这些检查补充服务层按看板划分的写入保护。当前结构中，`task_labels`、`task_dependencies`、
  `task_steps`、`task_execution_plans`、`task_runs`、`task_comments`、`signals` 和
  `task_attachments` 受按看板组合的外键保护。`signal_observations` 和 `task_events`
  保留可空的来源引用；诊断和导入流程仍会检查这些看板关系，作为损坏 JSONL 或原始 SQL
  输入的硬错误诊断层。
- `PRAGMA foreign_key_check` 的结果以硬错误 `consistency_issues[]` 呈现，并包含表名、
  行 ID、父表和外键索引。导入会在提交前运行同一门禁，出现违规时回滚。
- `consistency_errors` 非零时，`ok=false`。

本体台账诊断是只读的：

- `ontology_ledger_errors` 和 `ontology_ledger_warnings` 汇总硬错误和警告。
- `ontology_ledger_issues[]` 以 `severity`、`code`、`message` 和 `record_ids` 报告结构化问题。
- v12 及以上数据库要求具备 `label_ontology_observations`、`label_ontology_signals`、
  `label_ontology_actions`、`label_ontology_action_atom_effects` 和
  `label_ontology_action_signals`。
- 硬错误包括跨看板本体链接、孤立的操作信号或操作效果链接、缺失父级或取代引用、
  标签、提议或任务的看板不一致、信号取代环和操作父级环。错误非零时 `ok=false`。
- 警告只用于可重建或可由历史解释的软引用，例如某操作的 `result_atom_id` 所指向的当前
  `label_atoms` 行已在重建中消失。

派生层诊断是只读的：

- `outbox_pending`、`outbox_running` 和 `outbox_failed` 汇总 `index_outbox`。
- `derived_dirty_stores` 统计 `dirty=true` 的存储。
- `derived_error_stores` 统计存在 `last_error` 或发件箱失败项的存储。
- `derived_stores[]` 报告每个存储的 `store_name`、`schema_version`、`last_event_id`、
  `dirty`、`last_error`，以及该存储目标的待处理、运行中和失败发件箱数量。

`derived_stores[].last_event_id` 是存储级成功事件水位，不是看板本地水位。`dirty=true`
表示该存储在某个看板上仍有未完成的发件箱项，或最近一次更新失败。按看板同步或重建可以推进
水位；如果其他看板仍有待处理或失败工作，存储会继续保持脏状态。

这些字段不会让 Tantivy、Oxigraph 或 LanceDB 成为权威数据源。SQLite 仍是事实源，
脏的派生存储仍是可重建缓存。

### 15.2 检查点

```http
POST /api/v1/maintenance/checkpoint
```

运行 `PRAGMA wal_checkpoint(TRUNCATE)`，并返回 `busy`、`log_frames` 和
`checkpointed_frames`。

### 15.3 备份

当前不提供 HTTP 备份；请使用 CLI 备份命令。

---

## 16. Web 界面交互规则

1. 拖拽列时调用状态转换端点。
2. 普通字段编辑调用 `PATCH /tasks/{id}`。
3. Web 界面不显示 `claim_token`，调试模式除外。
4. 对 `running` 任务执行完成或阻塞操作时，若没有令牌，界面使用 `force=true` 并要求确认。
5. `blocked` 任务解除阻塞后的目标列由服务端返回，前端不要预设。
6. SSE 收到事件后，优先重新获取受影响任务，避免客户端状态机漂移。

### 16.1 信号评论

API 评论 DTO 使用 `kind: "signal"` 表示信号台账的反向链接评论。服务生成的反向链接会在
自然 JSON `metadata` 对象中包含 `type:"signal_link"`、`signal_id`、`observation_id`、
`signal_kind` 和 `signal_status`。通用信号评论的元数据保持开放且无损；客户端应把正文
作为可读的后备内容，只有完整的反向链接结构存在时才可链接到信号详情。


## 附录 A. 传输目录实现说明

本规范所列的每个 API 或 SSE 方法与路径，都以 `kanban-contract` 的端点描述目录作为唯一
实现来源。注册处理器时使用稳定的 `operation_id` 与 `adapter_id`；两者分别表示公开端点和
服务端运行时绑定，不是 Rust 类型名、函数地址或由 `stringify!` 推导出的值。
