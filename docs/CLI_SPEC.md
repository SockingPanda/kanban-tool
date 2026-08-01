# CLI 规范

默认二进制名称：`kanban`

CLI 是一等入口；它与 Tauri Desktop 和本机 API 共用由 `kanban-sqlite::service` 支撑的服务路径
和 SQLite 模式。

---

## 1. 全局选项

```bash
kanban [GLOBAL_OPTIONS] <COMMAND>
```

| 选项 | 说明 |
|---|---|
| `--db <path>` | 指定 SQLite 数据库；优先级高于环境变量、配置文件和 XDG 默认路径。 |
| `--board <slug-or-id>` | 显式指定当前看板，优先级最高。 |
| `--actor <name>` | 操作者名称，默认为操作系统用户名。 |
| `--locale <auto|system|zh-CN|en>` | 选择区域设置；省略、`auto` 或 `system` 时使用系统区域设置。当前只覆盖部分错误提示和依赖命令文案。 |
| `--json` | JSON 输出。 |

SQLite 数据库路径的解析顺序：

1. `--db <path>`。
2. `KANBAN_DB` 环境变量。
3. `KB_DB` 环境变量（兼容短名）。
4. 从当前目录向上查找最近的 `.kb/config.toml`，读取 `db = "<path>"`。
5. 用户全局配置 `$XDG_CONFIG_HOME/kanban/config.toml`，读取 `db = "<path>"`。
6. 回退到 XDG 数据默认路径，通常是 `~/.local/share/kb/kb.db`。

当前看板的解析顺序：

1. `--board <slug-or-id>`。
2. `KB_BOARD` 环境变量。
3. 从当前目录向上查找最近的 `.kb/config.toml`，读取 `board = "<slug>"`。
4. 回退到 `default`。

`kanban board use <board>` 会把当前目录写成项目级 `.kb/config.toml`；后续子目录自动继承该当前看板。该配置只选择本地项目的看板，不创建新数据库。
如果同一配置文件也包含 `db = "<path>"` 或 `[vector]`，`board use` 必须保留这些字段。配置中的相对数据库路径按配置文件所在目录解析；环境变量和 `--db` 中的相对路径按当前工作目录解析。

区域设置不改变 JSON 键、状态枚举、任务引用、ID、退出码或机器可读诊断信息。当前本地化
只覆盖部分运行时错误提示，以及依赖新增和移除的少量人类可读输出；初始化、任务、步骤、
配置等多数人类输出仍为英文，因此不能把该选项理解为完整界面翻译。选择顺序：

1. `--locale <auto|system|zh-CN|en>`。
2. `KANBAN_LOCALE`。
3. 系统区域设置。

`auto` / `system` 会按 `LC_ALL`、`LC_MESSAGES`、`LANG` 解析系统区域设置；当前只支持中文和英文。脚本和自动化应优先使用 `--json`，不要依赖人类可读文案。

### 1.1 查看配置

```bash
kanban config show [--json]
```

`config show` 输出当前 CLI 会使用的 SQLite 数据库路径、当前看板和区域设置，以及每个值的来源。该命令用于智能体或操作人员排查优先级，不会打开、初始化或创建 SQLite 数据库。

`--json` 输出使用常规 `{ "data": ... }` 封装，`data` 结构如下：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "db": {
    "value": "/path/to/kb.db",
    "source": { "kind": "project_config", "path": "/repo/.kb/config.toml", "key": "db" }
  },
  "board": {
    "value": "kanban-tool",
    "source": { "kind": "env", "name": "KB_BOARD" }
  },
  "locale": {
    "value": "zh-CN",
    "input": "auto",
    "source": { "kind": "flag", "name": "--locale" }
  }
}
```

`source.kind` 是脚本可依赖的 ASCII 枚举：

| `source.kind` | 含义 |
|---|---|
| `flag` | 来自显式 CLI 标志，例如 `--db`、`--board`、`--locale`。 |
| `env` | 来自环境变量，例如 `KANBAN_DB`、`KB_DB`、`KB_BOARD`、`KANBAN_LOCALE`。 |
| `project_config` | 来自最近的项目级 `.kb/config.toml`。 |
| `global_config` | 来自 `$XDG_CONFIG_HOME/kanban/config.toml`。当前只适用于数据库路径。 |
| `default` | 来自 CLI 默认值或回退值。 |

`locale.value` 是实际解析后的区域设置；当输入为 `auto` / `system` 时，`input` 保留原始选择，`value` 保留系统区域设置的解析结果。`db.value` 对显式标志和环境变量保留调用方传入的路径形态；配置中的相对数据库路径按配置文件所在目录解析。

### 1.2 帮助输出契约

`kanban --help` 和公开命令组的 `--help` 输出必须为每个公开命令或子命令提供一句简短用途说明；隐藏内部命令（例如 `__complete`）除外。`kanban` 无参数或公开命令组缺少子命令时，必须显示同一类简洁帮助，而不是只输出解析错误；这仍属于 clap 参数解析阶段，退出码为 2，且不输出运行期 JSON 错误封装。全局选项的帮助必须说明它们影响的是 SQLite 数据库、当前看板、操作者、区域设置或 JSON 输出，不改变 JSON 键、状态枚举或退出码契约。

面向智能体的关键输入面必须在命令帮助中优先展示安全路径：多行或对 shell 敏感的文本使用 `--description-file -`、`--body-file -`、`--metadata-json-file <PATH|->`、`--metadata-file <PATH|->` 或 `--input -`，避免 shell 展开或引号处理污染。危险、破坏性或容易误解的标志必须在帮助中说明语义，例如 `task archive --force` 绕过普通归档保护，`import --replace` 是明确用于备份恢复流程的替换式恢复入口；兼容性空操作标志必须明确写出其不执行额外操作。

对 `PATH|-` 文本输入（如 `--reason-file`、`--input`、`--body-file`、`--metadata-json-file`）与其变体，`kanban` 实现上约束单次输入上限为 1MiB。超过上限时返回 `invalid_input`，并在 `--json` 下通过 `error.message` 指明输入长度限制，CLI 端可用更高层分片策略。该约束覆盖标准输入与文件输入，目的是避免错误输入导致 CLI 服务路径资源异常。

顶层帮助和面向智能体的关键命令可以包含 `Examples:`，但示例必须保持短小、稳定，并与实际命令语义一致；不要把 CLI 规范的完整说明复制进帮助。CLI 帮助契约由 `crates/kanban-cli/tests/help.rs` 覆盖，防止公开命令行退化为空描述。

顶层 `kanban --help` 必须包含简洁的 `Error codes:` 小节，覆盖当前公开退出码，帮助操作人员在终端直接发现参数解析阶段与运行阶段的错误码边界。该小节是人类可读的发现入口；脚本仍应依赖 `--json` 下的 `error.code` 和 `error.exit_code`，不要解析帮助文案。

### 1.3 JSON 输出契约

所有公开 `--json` 输出使用顶层封装：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {},
  "meta": {}
}
```

`meta` 只在需要分页、详细信息或诊断信息时出现。`data` 可以是一个对象，也可以是对象数组；公共输出不得依赖裸元组、未命名数组位置、只有内部 ID 的临时数组，或只回显输入参数。命令需要表达关系、删除或当前选择时，应返回命名 DTO，例如 `edge.parent`/`edge.child`、`step`、`board`。任务类 DTO 必须带可复制的 `ref`、`id`、`board_id` 或 `board_slug` 中的必要身份字段。

`board current --json` 和 `board use --json` 的 `data.board` 是完整看板对象；调用方应读取 `data.board.slug`，不要把 `data.board` 当字符串。

#### JSON 错误输出

当 `--json` 已被 clap 成功解析，且错误发生在运行期服务或 I/O 路径时，CLI 向标准输出写入稳定的错误封装，并使用对应退出码：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "error": {
    "code": "not_found",
    "message": "未找到：board missing",
    "exit_code": 3
  }
}
```

`error.code` 是脚本可依赖的 ASCII 枚举；`message` 是本地化的人类可读说明；`exit_code` 与进程退出码一致。运行期 `--json` 错误不写标准错误。

`error.code` 不应依赖业务校验消息文案推断；普通业务层 `KanbanError::InvalidInput` / `InvalidStatus` 都返回稳定的 `invalid_input`。已通过 clap 解析后的用户配置 TOML 解析失败也属于 `invalid_input`：例如 `kanban --json config show` 读取格式错误的 `.kb/config.toml` 或 `$XDG_CONFIG_HOME/kanban/config.toml` 时，向标准输出写入运行期 JSON 错误、退出 2、不写标准错误，并且不打开、初始化或创建 SQLite 数据库。仅对缺少结构化分类的外层错误（I/O、路径、异常第三方文本），以及穿过 `InvalidInput` 的 SQLite 或维护锁哨兵值，使用降级文本分类作为补充，例如 `sqlite_busy`。

参数解析错误发生在 clap 解析阶段，仍由 clap 写入标准错误并退出 2；这类错误不输出 JSON 封装。没有 `--json` 时，运行期错误继续写入人类可读的标准错误。

### 1.3.1 JSONL / NDJSON 流式输出边界

JSONL/NDJSON 只适用于流式或面向记录的接口，例如可移植导出/导入、监视/事件流，或未来逐条输出的长流命令。该类输出必须满足：标准输出中的每一行都是独立有效的 JSON 对象，编码为 UTF-8，记录之间仅用换行符分隔；人类可读的诊断、进度、警告和运行期错误不得混入同一个标准输出数据流。

有限命令仍使用 `--json` 的 `{data, meta?}` 成功封装或 `{error:{code,message,exit_code}}` 运行期错误封装。JSONL/NDJSON 不替代有限命令封装，也不能成为未设计的全局 `--jsonl` 快捷方式。若某个命令支持 `--out -` JSONL 流，则它不得与 `--json` 共享标准输出；需要结构化错误时，必须在命令级定义流错误策略，并用逐行 JSON、标准输出/标准错误纯净性和退出码测试覆盖。

当前公开错误代码：

| `error.code` | 退出码 | 含义 |
|---|---:|---|
| `generic_error` | 1 | 未分类通用错误。 |
| `invalid_input` | 2 | 参数已通过 clap 解析，但业务输入、值域或校验无效。 |
| `not_found` | 3 | 看板、任务、标签、步骤、运行记录等对象未找到。 |
| `invalid_transition` | 4 | 状态机拒绝该转换，或必需执行计划/步骤未满足。 |
| `claim_conflict` | 5 | 并发领取，以及能被分类器识别的领取或心跳冲突。 |
| `dependency_blocked` | 6 | 依赖未完成导致任务不能进入 ready/running。 |
| `sqlite_busy` | 7 | SQLite 忙碌/锁定，或维护锁/运行锁造成阻塞。 |
| `integrity_check_failed` | 8 | `doctor`、导入或维护过程发现完整性或一致性硬错误。 |
| `storage_error` | 1 | 其它存储错误；不保证可按 SQLite 锁或完整性错误自动恢复。 |

当前完成、提交审核或阻塞时的 `claim token mismatch` 会归类为
`invalid_transition`（退出码 4），不是 `claim_conflict`。自动化调用方应以这里记录的
当前分类为准，不要把所有领取凭证错误都假定为退出码 5。

### 1.4 Shell 补全

```bash
kanban completions <shell>
kanban __complete <kind> [prefix]
```

`kanban completions <shell>` 向标准输出写入补全脚本。支持的 shell：

```text
bash | zsh | fish | powershell | elvish
```

所有受支持的 shell 都会生成静态命令和选项补全。Bash 与 zsh 脚本还包含动态钩子，
会调用隐藏的内部辅助命令 `kanban __complete`，获取由数据库提供的候选值：

- 任务、评论、事件、运行记录和依赖命令所需的任务引用；
- `--board` 和看板身份参数所需的看板 slug；
- `--status` 所需的状态值；
- `comment add --kind` 所需的评论类型值（`note`、`decision`、`signal`）。

`kanban __complete` 是供 shell 脚本和测试使用的内部辅助命令，结果按换行符分隔。它接受：

```text
task-ref | dependency-task-ref | board | status | comment-kind
```

为满足补全场景，该辅助命令必须保持静默：数据库文件缺失、数据库未初始化、看板配置缺失，
或读取/查询失败时，都成功退出且不返回候选值、不写标准错误。生成静态补全脚本本身
不会打开或创建 SQLite 数据库。

### 1.5 Codex 钩子

```bash
kanban hook codex install [--handler-command <command-prefix>] [--timeout 30] [--record-signals] [--json]
kanban hook codex status [--json]
kanban hook codex uninstall [--json]
kanban hook codex handle failure [--record-signals]
kanban hook codex handle task-create
```

`kanban hook codex` 管理一组 Codex 生命周期钩子，为智能体提供与看板相关的反馈。
钩子安装到 Codex 用户配置路径：`$CODEX_HOME/hooks.json`；未设置 `CODEX_HOME` 时
使用 `~/.codex/hooks.json`。该功能不提供项目级安装模式，因为 kanban 旨在跨工作区
提供一致的 CLI 感知行为。

钩子提示词从用户的 kanban 配置路径读取：
`$XDG_CONFIG_HOME/kanban/codex-hooks.json`，通常是
`~/.config/kanban/codex-hooks.json`。若文件不存在，`install` 会用中文默认提示词创建，
但不会覆盖现有文件。若提示词文件缺失、格式错误、使用不支持的 `version`，或绑定指向
不存在的提示词别名，处理程序会回退到内置中文默认值，而不会让 Codex 钩子失败。

`install` 在匹配器 `^Bash$` 下添加两个受管的 `PostToolUse` 命令钩子：
一个处理失败的 `kanban ...` 命令记录，另一个为成功的
`kanban task create ...` 提供后续建议。受管命令前缀默认为
`kanban hook codex handle`；安装的命令如下：

```bash
kanban hook codex handle failure --installed-by kanban-hook-codex [--record-signals]
kanban hook codex handle task-create --installed-by kanban-hook-codex
```

`uninstall` 只删除带隐藏标记 `--installed-by kanban-hook-codex` 的钩子，并保留
无关的用户钩子。重复运行 `install` 是幂等的：写入新钩子前会替换先前的受管钩子。

`handle failure` 和 `handle task-create` 是内部钩子命令。它们从标准输入读取
Codex 钩子 JSON，不输出内容，或直接输出如下 Codex 钩子响应对象：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{"systemMessage":"检测到 kanban CLI 命令失败。\n\n命令：kanban task list --bad-flag\n退出码：2\n\n继续调整。修正后继续当前任务，并在确有必要时记录后续工作。"}
```

`handle` 子命令有意不使用常规 `{ "data": ... }` JSON 封装，因为 Codex 会直接消费
钩子的标准输出。公开管理命令 `install`、`status` 和 `uninstall` 则使用常规
`--json` 封装。

提示词配置模式：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "version": 1,
  "codex_hooks": {
    "bindings": {
      "failure": "failure.zh-default",
      "task_create": "task_create.zh-default"
    },
    "prompts": {
      "failure.zh-default": "检测到 kanban CLI 命令失败。\n\n命令：{{command}}\n退出码：{{exit_code}}\n\n继续调整。修正后继续当前任务，并在确有必要时记录后续工作。",
      "task_create.zh-default": "检测到 kanban task 创建成功。\n\n命令：{{command}}\n任务：{{task_ref}}\n\n请考虑为该 task 执行 label/signal follow-up；需要标签时先运行 `kanban label suggest {{task_ref}} --json`。不要自动写 label ontology，除非已有完整 suggestion snapshot 和明确 decision payload。"
    }
  }
}
```

支持的占位符有意保持精简：

- `failure`：`{{command}}`、`{{exit_code}}`；
- `task_create`：`{{command}}`、`{{task_ref}}`。

`stderr` 和 `stdout` 不是提示词占位符。使用 `handle failure --record-signals` 时，
它们仍会作为有界的内部证据保存在所记录的通用信号中。

V1 行为：

- 非 `Bash` 工具和不调用 `kanban` 的 Bash 命令都是空操作；
- `handle failure` 只报告失败的 `kanban ...` 命令，提示词来自
  `codex-hooks.json` 或内置中文默认值；
- `handle failure --record-signals` 还会记录一个通用信号，其中包含
  `kind="agent_cli_failure"`、`source="kanban-hook-codex"` 和有界的命令证据；
- `handle task-create` 只报告成功的 `kanban task create ...` 命令，并使用
  `codex-hooks.json` 或内置中文默认值渲染标签/信号后续提示；
- 钩子绝不会静默启动 Codex 原生子智能体，也不会自动写入标签本体。它只注入建议；
  当前 Codex 会话必须自行决定是否启动原生智能体或记录本体观察。

---

## 2. 退出码

| 代码 | 含义 |
|---:|---|
| 0 | 成功。 |
| 1 | 通用错误或未分类存储错误。 |
| 2 | clap 参数错误，或运行期校验/无效输入。 |
| 3 | 未找到对象。 |
| 4 | 非法状态转换，或必需执行计划/步骤未满足。 |
| 5 | 并发领取，以及能被分类器识别的领取或心跳冲突。 |
| 6 | 依赖阻塞。 |
| 7 | SQLite 忙碌/锁定，或维护锁/运行锁阻塞。 |
| 8 | 完整性检查失败或一致性硬错误。 |

---

## 3. 初始化

### 3.1 `kanban init`

初始化本地数据库、默认看板和默认列。该命令是幂等的；重复执行只会应用缺失的迁移并确保默认数据存在，不会重置或覆盖已有任务数据。`--force` 是兼容旧脚本的空操作，不改变 `init` 行为。

```bash
kanban init
kanban init --db .kb/kb.db
kanban init --force
```

`--force` 是已弃用的兼容性空操作：保留用于兼容旧脚本，不改变 `init` 行为，不执行重置或覆盖，也不会绕过迁移或模式校验。

输出：

```text
Initialized Kanban database at ~/.local/share/kb/kb.db
Default board: default
```

JSON：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "db_path": "/home/user/.local/share/kb/kb.db",
    "board_id": "b_01HX...",
    "board_slug": "default"
  }
}
```

---

## 4. 看板命令

### 4.1 列出看板

```bash
kanban board list [--include-archived]
```

### 4.2 创建看板

```bash
kanban board create <slug> --name <name> [--description <text>]
```

示例：

```bash
kanban board create agent-work --name "智能体工作"
```

### 4.3 查看看板

```bash
kanban board show <slug>
```

### 4.4 选择看板

```bash
kanban board use <slug-or-id>
```

写入：

```toml
board = "agent-work"
```

写入当前目录的 `.kb/config.toml`。

### 4.5 查看当前看板

```bash
kanban board current
```

应用 `--board`、`KB_BOARD`、项目配置和回退优先级后，显示最终解析出的当前看板。
看板解析与数据库路径解析相互独立：`--db` / `KANBAN_DB` / `KB_DB` 决定打开哪个
SQLite 数据库，`--board` / `KB_BOARD` / `.kb/config.toml` 中的 `board` 则决定选择
该数据库里的哪个看板。

### 4.6 归档看板

```bash
kanban board archive <slug>
```

除非传入 `--include-archived`，否则 `kanban board list` 不显示已归档看板。系统会拒绝
向已归档看板进行普通任务写入。只要能显式解析出任务或看板，仍可通过任务、事件、运行记录
和评论历史命令读取审计历史。若看板中仍有 `running` 工作，归档请求会被拒绝；应先完成、
阻塞或回收这些工作。

---

## 5. 任务命令

### 5.1 创建任务

```bash
kanban task create <title> [OPTIONS]
```

选项：

| 选项 | 说明 |
|---|---|
| `--description <text>` | Markdown 描述。 |
| `--description-file <PATH|->` | 从文件或标准输入（`-`）读取 Markdown 描述；与 `--description` 互斥。推荐用于多行或包含 `$`、反引号、JSON 等对 shell 敏感的文本。 |
| `--status <status>` | 显式指定初始状态：`triage` / `todo` / `scheduled` / `ready`。 |
| `--assignee <name>` | 负责人或工作者配置名称。 |
| `--priority <int>` | 优先级 `0..3`：`0` = P0 事故、阻塞项或必须立即处理；`1` = P1 近期重点；`2` = P2 重要后续；`3` = P3 普通待办、低优先级或默认值。非法值会被拒绝。 |
| `--scheduled-at <epoch_ms>` | 计划时间，Unix 纪元毫秒数。 |
| `--due-at <epoch_ms>` | 截止时间，Unix 纪元毫秒数。 |
| `--max-retries <n>` | 工作者失败或回收后最多重试次数。 |
| `--label <name>` | 创建时附加已存在标签，可重复；若看板内缺少任一标签，整个创建操作都会被拒绝。 |
| `--metadata <json>` | 扩展 JSON。 |
| `--metadata-file <PATH|->` | 从文件或标准输入（`-`）读取扩展 JSON；与 `--metadata` 互斥。推荐用于避免 JSON 的 shell 引号处理问题。 |

优先级只表达相对重要性和排序，不表示任务可以被领取。只有 `ready` 才表示任务已被显式
放入可执行队列；普通 `ready` 任务通常仍应是 P1/P2/P3，不能为了表示“下一批可做”
而全部标成 P0。P0 只用于事故、当前目标的阻塞项或必须立即处理的任务；若 P0 任务仍缺
规格、排期未到或依赖未完成，它仍保持 `triage` / `scheduled` / `todo`，不能被领取。

`task create` 可以请求 `--status ready`，但新任务创建时尚无执行计划。服务会把任务实际
保存为 `todo`；查询和响应会把执行计划状态派生为 `unplanned`，并不会为此写入计划行。
添加第一个步骤或执行
`task step not-required` 后，服务才会结合规格、排期和依赖等其他保护条件重新计算状态。
显式请求 `scheduled` 时必须同时提供 `--scheduled-at`；显式请求 `ready` 时必须有非空
描述，且排期不能位于未来。

示例：

```bash
kanban task create "修复 claim 队列阻断回归" --priority 0
kanban task create "实现状态机" --description "补齐状态转换和测试" --priority 1 --status ready
kanban task create "补充文档示例" --priority 2
kanban task create "明早检查报告" --scheduled-at 1780640400000
kanban task create "修复 API 回归" --label backend --label p1
```

`--label` 只绑定当前看板中已存在的标签身份。名称会先去除首尾空白；空白名称会被拒绝。
任一标签缺失时，整个创建操作返回无效输入，且不会写入 `tasks`、`labels`、
`task_labels` 或 `task_events`。需要新的词汇身份时，先显式运行
`kanban label create`，或使用 `kanban label add --create-missing` 这类明确的身份
创建入口；任务创建本身没有自动创建缺失标签的模式。

人类可读输出：

```text
agent-work#12 [todo] P1 实现状态机 · plan: unplanned · steps: 0/0
```

JSON 输出：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "id": "t_01HX...",
    "board_id": "b_01HX...",
    "board_slug": "agent-work",
    "ref": "agent-work#12",
    "seq": 12,
    "status": "todo",
    "title": "实现状态机",
    "labels": []
  }
}
```

### 5.2 列出任务

```bash
kanban task list [OPTIONS]
```

选项：

| 选项 | 说明 |
|---|---|
| `--status <status>` | 按状态过滤，可重复。 |
| `--assignee <name>` | 按负责人过滤。 |
| `--label <name>` | 按标签名称或 ID 过滤，可重复；多个标签使用 AND 语义。 |
| `--search <query>` | 模糊搜索标题和描述；形似任务引用的查询按精确匹配处理。 |
| `--include-archived` | 包含已归档任务。 |
| `--limit <n>` | 限制数量。 |
| `--offset <n>` | 分页偏移。 |
| `--sort <field>` | `seq` / `title` / `status` / `position` / `priority` / `assignee` / `scheduled_at` / `due_at` / `created_at` / `updated_at`。降序可用 `<field>_desc`，也兼容 API 风格 `-<field>`。`priority` 按 P0 → P3 排序；`priority_desc` / `-priority` 按 P3 → P0 排序。 |
| `--plan-needed` | 只列出执行计划仍为 `unplanned` 的活动任务。 |
| `--has-steps` | 只列出至少有一个步骤的任务。 |
| `--incomplete-required-steps` | 只列出存在未完成必需步骤的任务。 |
| `--plan-filter <filter>` | 可重复：`plan-needed` / `has-steps` / `incomplete-required-steps`。 |

优先级排序不会把工作提升为 `ready`；它只对所选结果集中的任务排序。

`--search` 对任务引用形状使用精确匹配，而不是文本包含匹配：
纯数字 `12`、`#12` 匹配当前看板内的序号；`board#12` / `board/#12`
只在该看板与当前列表请求的看板相同时匹配；`t_...` 只匹配当前列表请求看板
内的任务 ID。其他文本仍执行标题和描述的模糊搜索。

示例：

```bash
kanban task list
kanban task list --status ready --status running
kanban task list --label backend --label p1
kanban task list --assignee agent-default --json
kanban task list --plan-needed
kanban task list --plan-filter incomplete-required-steps
```

### 5.3 查看任务

```bash
kanban task show <task_ref>
kanban task show <task_ref> --details
```

默认人类可读输出仍是紧凑的单行任务摘要；默认摘要便于快速扫描，保留可复制的引用、状态、
优先级、标题、标签，以及必要的计划/步骤信号，不默认展示内部 `t_...` ID：

```text
agent-work#12 [ready] P1 实现状态机 · plan: planned · steps: 0/0
```

`--details` 改变人类可读输出，按 `Task`、`Description`、`Plan`、`Schedule`、
`Timestamps`、`Execution`、`Result`、`Metadata` 分组显示易读字段列表。可用时包含
任务引用、ID、状态、标题、完整多行描述、负责人、优先级、标签、`scheduled_at`、
`due_at`、`created_at`、`updated_at`、执行计划状态、必需/可选步骤数量、领取信息、
运行记录、结果、元数据以及其他任务快照字段。
如果该任务有标签本体信号，详细输出还会追加紧凑的 `ontology_summary`，列出信号、
状态、降级、过期和操作数量、老化时间，以及少量信号 ID 示例。

`task show <task_ref> --json` 默认只返回 `{"data": TaskRecord}`。带 `--details`
时，`data` 仍是相同的 `TaskRecord`，但封装会包含
`meta.details.ontology_summary`；没有本体信号时该字段为 `null`。该摘要
只读，不改变任务、标签或本体信号状态。需要完整审核队列时继续使用
`label ontology list/show/review`。

`task_ref` 支持：

- `t_...`：全局任务 ID，忽略当前看板。
- `12`：当前看板内的序号。
- `#12`：当前看板内的序号；在 shell 中需要引号，例如 `'#12'`。
- `agent-work#12`：显式看板 slug + 序号。
- `agent-work/#12`：兼容别名/#序号形式。
- `b_01HX...#12`：显式看板 ID + 序号。

裸 `12` / `#12` 依赖当前看板；显式 `board#seq` 和 `t_...` 可跨当前看板使用。
当前版本会拒绝跨看板依赖。

### 5.4 更新任务字段

```bash
kanban task update <task_ref> [OPTIONS]
```

允许更新的选项：

| 选项 | 说明 |
|---|---|
| `--title <text>` | 更新标题。 |
| `--description <text>` | 更新描述。 |
| `--description-file <PATH|->` | 从文件或标准输入读取描述；与 `--description` 互斥。 |
| `--assignee <name>` | 更新负责人。 |
| `--clear-assignee` | 清空负责人；若同时提供 `--assignee`，以清空为准。 |
| `--priority <int>` | 更新优先级。 |
| `--scheduled-at <epoch_ms>` | 更新计划时间。 |
| `--clear-scheduled-at` | 清空计划时间；若同时提供 `--scheduled-at`，以清空为准。 |
| `--due-at <epoch_ms>` | 更新截止时间。 |
| `--clear-due-at` | 清空截止时间；若同时提供 `--due-at`，以清空为准。 |
| `--max-retries <n>` | 更新最大重试次数。 |
| `--clear-max-retries` | 清空最大重试次数；若同时提供 `--max-retries`，以清空为准。 |
| `--metadata <json>` | 更新扩展 JSON。 |
| `--metadata-file <PATH|->` | 从文件或标准输入读取扩展 JSON；与 `--metadata` 互斥。 |
| `--expected-lock-version <version>` | 要求任务当前 `lock_version` 与给定值一致，不一致时拒绝更新。 |

不允许通过更新命令修改状态；状态必须通过转换命令修改。允许更新的字段仍由共享服务路径
处理，因此修改描述、`scheduled_at` 等会影响规格或排期的字段后，服务会根据规格、
排期和当前依赖重新计算活动任务的目标状态并写入对应事件。依赖边通过 `kanban dep`
命令修改；`max_retries` 只更新重试策略，不触发状态重算。

示例：

```bash
kanban task update 12 --priority 1
kanban task update t_01HX --description "新的规格"
kanban task update t_01HX --description-file - <<'EOF'
新的多行规格，保留 $VAR、$(command)、反引号和 JSON 字面量。
EOF
kanban task update t_01HX --max-retries 2
kanban task update t_01HX --clear-max-retries
```

---

## 6. 状态转换命令

### 6.1 提升为可执行

```bash
kanban task promote <task_ref>
```

手动尝试 `todo/scheduled -> ready`。

### 6.2 启动/领取

```bash
kanban task start <task_ref> [OPTIONS]
kanban task claim <task_ref> [OPTIONS]
```

`start` 是 `claim` 更便于人类理解的别名。

选项：

| 选项 | 说明 |
|---|---|
| `--ttl-ms <ms>` | 领取有效期（TTL）。默认 300000。 |

输出：

```text
Claimed t_01HX... token=claim_01HX...
```

JSON 返回规范领取快照：`data.task` 是闭合的 `ApiTask`，`data.run`
是闭合的 `ApiRun`，令牌只允许出现在顶层 `data.claim_token`。下面仅节选身份
与状态字段；实际对象还包含各自模式声明的其余字段：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task": {
      "id": "t_01HX...",
      "status": "running",
      "current_run_id": "r_01HX..."
    },
    "run": {
      "id": "r_01HX...",
      "task_id": "t_01HX...",
      "status": "running"
    },
    "claim_token": "claim_01HX...",
    "claim_expires_at": 1717520000000
  }
}
```

### 6.3 心跳

```bash
kanban task heartbeat <task_ref> --claim-token <token>
```

选项：

| 选项 | 说明 |
|---|---|
| `--ttl-ms <ms>` | 延长 TTL。 |

显式心跳 API 保持兼容。除此之外，`running` 任务的有效任务级活动事件也会隐式刷新租约，
可作为存活信号；该隐式刷新不会再写 `task.heartbeat` 事件。看板级事件或没有 `task_id`
的事件不触发续租。

### 6.4 完成

```bash
kanban task done <task_ref> [--claim-token <token>] [--force]
kanban task complete <task_ref> [--claim-token <token>] [--force]
```

选项：

| 选项 | 说明 |
|---|---|
| `--claim-token <token>` | 从 `running` 完成时需要匹配当前领取；从 `review` 完成时可省略。 |
| `--force` | 只绕过 `running` 的领取匹配，不能绕过必需步骤守卫；仅供本地人工修复使用。 |

### 6.5 提交审核

```bash
kanban task review <task_ref> [--claim-token <token>] [--force]
```

使任务从 `running` 转为 `review`。正常路径需要匹配领取凭证；`--force` 只绕过领取守卫。

### 6.6 阻塞

```bash
kanban task block <task_ref> (<reason>|--reason-file <PATH|->)
```

选项：

| 选项 | 说明 |
|---|---|
| `--claim-token <token>` | 阻塞 `running` 任务时需要。 |
| `--force` | 强制阻塞。 |
| `--reason-file <PATH|->` | 从文件或标准输入（`-`）读取阻塞原因；与位置参数 `<reason>` 互斥。 |

### 6.7 解除阻塞

```bash
kanban task unblock <task_ref>
```

不会盲目进入 `ready`，而是根据规格、排期和依赖重新计算目标状态。

### 6.8 重新打开

```bash
kanban task reopen <task_ref> (--reason <text>|--reason-file <PATH|->)
```

只允许重新打开 `done` 任务，原因必填且不能为空，可用 `--reason-file <PATH|->`
从文件或标准输入读取；它与行内 `--reason` 互斥。重新打开会清空
`completed_at`，保留 `result_summary` / 自然 JSON `result`（持久层仍存于
`result_json`），并按规格、排期、依赖和执行计划就绪情况重新计算目标状态。

如果被重新打开的任务是其他任务的依赖父项，直接子任务中仅
`triage|todo|scheduled|ready` 会重新计算；`running|blocked|review|done|archived`
不会被隐式改写。

### 6.9 回收

```bash
kanban task reclaim --expired
kanban task reclaim
```

当前 CLI 回收当前看板内已过期的领取；裸 `kanban task reclaim` 与
`kanban task reclaim --expired` 等价。
JSON 输出固定为 `{"data":{"reclaimed":<u64>}}`，且拒绝未声明字段。

### 6.10 归档

```bash
kanban task archive <task_ref>
```

选项：

| 选项 | 说明 |
|---|---|
| `--force` | 允许归档 `running` 任务，并关闭当前运行记录。 |

---

### 6.11 步骤/执行计划

```bash
kanban task step list <task_ref>
kanban task step add <task_ref> <title> [--body <text>|--body-file <PATH|->] [--link-task <task_ref>] [--position <n>] [--required|--optional]
kanban task step update <task_ref> <step_ref> [--title <text>] [--body <text>|--body-file <PATH|->|--clear-body] [--link-task <task_ref>|--unlink-task] [--position <n>] [--required|--optional]
kanban task step done <task_ref> <step_ref> (--note <text>|--note-file <PATH|->)
kanban task step skip <task_ref> <step_ref> (--reason <text>|--reason-file <PATH|->)
kanban task step reopen <task_ref> <step_ref> (--reason <text>|--reason-file <PATH|->)
kanban task step remove <task_ref> <step_ref>
kanban task step not-required <task_ref> (--reason <text>|--reason-file <PATH|->)
```

步骤是执行计划的一等结构化项目。它可以是纯文本步骤，也可以通过
`--link-task` 引用同一看板内的普通任务作为上下文。链接任务不等于依赖，
链接任务的状态不会自动完成步骤。步骤自身的状态是 `todo`、`done` 或
`skipped`。

`step_ref` 支持步骤 ID，也支持父任务列表里的 `S<n>` 序号。`add` 默认创建
必需步骤；`--required` / `--optional` 互斥。供人类使用的规范形式是不带值的标志，
但针对该标志，CLI 也接受有界的智能体生成值：`--required true`、
`--required=false`，以及对应的 `--required=true` / `--required false` 形式。
只有字面量 `true` / `false` 会被当作布尔值消费；`--required` 之后的普通位置文本
仍是位置参数，任何其他额外值仍会触发解析错误。`--body-file <PATH|->` 从文件或
标准输入读取长正文，与 `--body` 互斥；`update --clear-body` 也与 `--body-file`
互斥。`update` 只有在显式传入 `--required` 或 `--optional` 时才改变是否必需。
`done`、`skip` 和 `reopen` 必须记录说明文本。`--note-file <PATH|->` 和
`--reason-file <PATH|->` 从文件或标准输入读取较长的备注/原因，分别与行内
`--note` / `--reason` 互斥。

人类可读列表输出示例：

```text
Execution plan: planned
Required steps: 1/2 done-or-skipped
Optional steps: 1

S1 step_01HX... [done] required pos=1024 编写测试
S2 step_01HY... [todo] required pos=2048 link=default#13 验证桌面界面
S3 step_01HZ... [todo] optional pos=3072 发布说明
```

`task step not-required` 只在没有步骤时可用；它记录原因并解除 `ready`/领取的
执行计划门禁。已有步骤的任务不能标记为 `not_required`。

---

## 7. 依赖命令

```bash
kanban dep add <parent_ref> <child_ref>
kanban dep remove <parent_ref> <child_ref>
kanban dep list <task_ref>
```

`--json` 输出使用已补全信息的依赖 DTO。`dep list --json` 返回以所查询任务为中心的快照：

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
    "parents": [
      {
        "id": "t_parent",
        "board_id": "b_default",
        "board_slug": "default",
        "ref": "default#1",
        "title": "父任务",
        "status": "done"
      }
    ],
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

`dep add --json` 和 `dep remove --json` 返回：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "edge": { "parent": {}, "child": {} },
    "dependencies": { "task": {}, "parents": [], "children": [], "edges": [] }
  }
}
```

常用 jq：

```bash
kanban dep list default#2 --json | jq -r '.data.edges[] | "\(.parent.ref) -> \(.child.ref)"'
```

添加和移除依赖的人类可读输出以中文为主：

```text
已添加依赖：default#1 -> default#2
已移除依赖：default#1 -> default#2
```

添加依赖后：

- 如果子任务当前是 `ready` 且父任务未完成（不是 `done` 或 `archived`），子任务降级为 `todo`。
- 父任务完成、归档或依赖移除后，子任务保持 `todo`；需要
  `kanban task promote <task_ref>` 才显式进入 `ready`。归档父任务不会删除依赖边。
- 父任务从 `done` 重新打开后，直接子任务中仅 `triage|todo|scheduled|ready`
  会按就绪条件重算；`running|blocked|review|done|archived` 不会被隐式改写。
- 重复添加同一父任务/子任务边是幂等空操作：不追加新的 `dependency.added` 事件，
  也不再次触发子任务状态重算。
- 如果产生环，返回 `invalid_input`，退出码为 2。
- 当前版本拒绝跨看板依赖，即使父任务/子任务通过全局 `t_...` 或显式
  `board#seq` 解析成功。

`task list/show --json` 返回派生依赖字段：`dependency_blocked`
和 `unfinished_parent_count`。未完成父任务指状态不是 `done` 或 `archived` 的父任务；
这些字段用于区分仍被未完成父任务阻塞的 `todo`，与已解除依赖但尚未人工提升的 `todo`。

---

## 8. 标签命令

```bash
kanban label list
kanban label create <name> [--color <color>]
kanban label delete <label> [--force] [--json]
kanban label bootstrap <task_ref> <label> [--description <text>] [--applies-when <text>]... [--excludes-when <text>]... [--positive-example <text>]... [--negative-example <text>]... [--verify] [--min-verify-score 0.50] [--vector-config <toml>] [--json]
kanban label add [--create-missing] <task_ref> <label>...
kanban label remove <task_ref> <label>
kanban label semantics list [--json]
kanban label semantics show <label> [--json]
kanban label semantics upsert <label> [--expected-semantics-hash <hash>] [--replace] [--reason <text>|--reason-file <PATH|->] [--source-signal <signal_id>]... [--description <text>] [--applies-when <text>]... [--excludes-when <text>]... [--positive-example <text>]... [--negative-example <text>]... [--remove-applies-when <text>]... [--remove-excludes-when <text>]... [--remove-positive-example <text>]... [--remove-negative-example <text>]... [--json]
kanban label semantics delete <label> --expected-semantics-hash <hash> (--reason <text>|--reason-file <PATH|->) [--json]
kanban label atoms list [--json]
kanban label atom explain <atom-id-or-content-hash> [--json]
kanban label atom-index status [--vector-config <toml>] [--json]
kanban label atom-index rebuild [--vector-config <toml>] [--json]
kanban label atom-index query <text> [--polarity positive|negative] [--limit 24] [--vector-config <toml>] [--json]
kanban label suggest <task_ref> [--limit 5] [--candidate-limit 32] [--atom-limit 80] [--max-selected-labels 4] [--min-score 0.15] [--vector-config <toml>] [--json]
kanban label propose <task_ref> [--proposal-json <path>] [--source-signal <signal_id>]... [--allow-retarget] [--retarget-reason <text>|--retarget-reason-file <PATH|->] [--actor-type user|agent] [--agent-type <type>] [--limit 5] [--candidate-limit 32] [--atom-limit 80] [--max-selected-labels 4] [--min-score 0.15] [--vector-config <toml>] [--json]
kanban label proposals list [--task <task_ref>] [--status proposed|accepted|rejected] [--json]
kanban label proposals show <proposal_id> [--json]
kanban label proposals accept <proposal_id> [--reason <text>|--reason-file <PATH|->] [--source-signal <signal_id>]... [--allow-retarget] [--retarget-reason <text>|--retarget-reason-file <PATH|->] [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label proposals reject <proposal_id> [--reason <text>|--reason-file <PATH|->] [--json]
kanban label ontology record <task_ref> --input <path|-> [--suggestion-snapshot <path|-> | --capture-suggest] [--limit 5] [--candidate-limit 32] [--atom-limit 80] [--max-selected-labels 4] [--min-score 0.15] [--vector-config <toml>] [--json]
kanban label ontology list [--status open|confirmed|resolved|rejected|superseded]... [--kind false_negative|false_positive|vocabulary_gap|name_issue|boundary_issue|structure_issue]... [--task <task_ref>] [--label <label>] [--proposed-label <name>] [--include-all] [--limit 100] [--json]
kanban label ontology show <signal_id> [--json]
kanban label ontology review [--group-by label|candidate-atom|proposed-label|cluster] [--include-all] [--limit 100] [--json]
kanban label ontology quality [--sample-limit 20] [--json]
kanban label ontology confirm <signal_id>... (--reason <text>|--reason-file <PATH|->) [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology reject <signal_id>... (--reason <text>|--reason-file <PATH|->) [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology supersede <signal_id>... --by <signal_id> (--reason <text>|--reason-file <PATH|->) [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology resolve <signal_id>... --no-change (--reason <text>|--reason-file <PATH|->) [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology apply atom <signal_id>... --label <label> --kind applies-when|positive-example|excludes-when|negative-example (--text <text>|--text-file <PATH|->) (--reason <text>|--reason-file <PATH|->) [--allow-retarget] [--retarget-reason <text>|--retarget-reason-file <PATH|->] [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology revert <action_id> (--reason <text>|--reason-file <PATH|->) [--expected-current-hash <hash>] [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology validate <action_id> --status passed|failed|partial (--reason <text>|--reason-file <PATH|->) --input <PATH|-> [signal_id]... [--actor-type user|agent] [--agent-type <type>] [--json]
kanban label ontology validate <action_id> --trusted --status passed|failed|partial (--reason <text>|--reason-file <PATH|->) [signal_id]... [--positive-control <TASK_REF>]... [--positive-control-waiver <REASON>|--positive-control-waiver-file <PATH|->] [--vector-config <toml>] [--limit 5] [--candidate-limit 32] [--atom-limit 80] [--max-selected-labels 4] [--min-score 0.15] [--actor-type user|agent] [--agent-type <type>] [--json]
```

`label atom-index status`、`rebuild` 和 `query` 复用向量 TOML 解析规则：
显式 `--vector-config`/`--config` 优先，其次是最近项目的 `.kb/config.toml`，
最后是全局配置。只有显式传入 `--vector-config` 时，辅助进程参数才会附带该值；
省略时由辅助进程按默认配置解析。

标签语义、提案和本体命令中的 `--reason-file <PATH|->`、
`--retarget-reason-file <PATH|->`、`--text-file <PATH|->` 和
`--positive-control-waiver-file <PATH|->` 从文件或标准输入读取对应长文本，并与同名
行内参数互斥。`label atom-index query <text>` 的 `<text>` 是短查询标量，不提供
文件输入；需要持久本体证据时使用 `label ontology record --input <path|->`
或 `label ontology validate --input <PATH|->`。

`label create` 在当前看板作用域内创建标签；如果同一看板已存在同名标签，则返回
已有标签。`label add` 接受任务引用和一个或多个标签名称；默认只绑定任务所属看板上
已经存在的规范标签。缺失标签会返回无效输入，并提示先用 `label create`、
`label bootstrap`、提案/采用路径创建，或在明确接受只创建规范身份的情况下传入
`--create-missing`。`--create-missing` 只创建 `labels` 身份并绑定任务，不生成
`label_semantics` 或 `label_atoms`；JSON 输出改为
`{ "task": <TaskRecord>, "created_labels": [...] }`。
`label remove` 接受任务引用和标签名称或 ID。空白标签名称会被拒绝。

`label delete <label>` 删除当前看板上的规范标签身份，区别于
`label remove <task_ref> <label>` 的任务级解绑。标签身份的增删改查不属于
本体账本；创建/删除只写普通看板/任务事件，不写本体变更操作。默认情况下，如果标签
仍绑定任何任务，系统会拒绝删除并报告绑定数量；显式传入 `--force` 时，只移除任务绑定
后删除空标签身份。若标签仍有
`label_semantics` 或 `label_atoms`，即使传 `--force` 也会拒绝；必须先用
`label semantics delete --expected-semantics-hash <hash> --reason <text>` 清空语义。
JSON 返回 `{ "label": <LabelRecord>, "forced": bool, "removed_task_bindings": n, "removed_semantics": false, "removed_atoms": 0 }`。
删除规范标签不改变任务
状态；被删除标签会从 `label list`、`task show/list` 的标签和后续建议事实
中消失。

标签变更对任务—标签关联保持幂等。只有关联实际变化时，才追加
`task.label.added` / `task.label.removed` 事件；该操作不改变任务状态。
批量 `label add` 会先验证所有标签名称；如果任一标签为空白、非法或缺失且未传
`--create-missing`，不会创建规范标签，也不会留下部分任务—标签绑定。
显式创建模式与添加单个标签相同，只创建缺失的规范身份，并在输出中列出本次新建的标签。

`label bootstrap` 是一次性新标签采用辅助命令：在同一事务内创建当前任务所属看板上
缺失的规范标签，或复用尚无语义的同名标签；写入该标签的 `label_semantics`，同步重建
SQLite `label_atoms`，将派生的标签原子向量索引标记为脏，并把该标签绑定到任务。
`<label>` 按名称解析；空白名称会被拒绝。语义输入会去除首尾空白并丢弃空白值，
且必须至少提供 `description` 或一个非空
语义数组值。

引导操作默认不会覆盖已有 `label_semantics`。如果同名标签已经有语义，
命令会失败，并要求改用专用语义变更或提案/采用路径；重复执行同一任务/标签时，
只有目标标签仍无语义才会保持任务—标签绑定幂等。JSON
返回 `{ "task": <TaskRecord>, "semantics": <LabelSemanticsRecord>, "verification": null|<Verification> }`。

当前轻量 CLI 构建已把标签建议/提案、引导阶段校验和标签原子状态/重建/查询接到向量辅助
子进程适配器；`kanban vector ...` 仍保留原始分块/标签原子查询入口，辅助进程内部用
标签原子专用命令处理 `lancedb_label_atoms`，不复用分块存储状态来伪装标签原子状态。

传入 `--verify` 或 `--vector-config <toml>` 时，CLI 使用提交前的分阶段校验：
先在规范数据库事务外读取当前任务、目标标签状态和看板本体摘要，并在隔离的临时原子存储
中加载当前原子与候选原子。随后对来源任务运行非降级的 `label suggest`，要求新标签
出现在 `selected_labels` 或 `candidates` 中，且分数至少达到
`--min-verify-score`（默认 `0.50`）。重建、建议、阈值、提供程序或临时存储失败时，
不会写入规范标签、语义、原子、任务—标签绑定、本体操作、事件或脏标记。如果向量辅助
进程/提供程序不可用，会返回明确的校验错误；需要离线验收时，也可改走外部证明
`--input` 路径。

验证通过后 CLI 才开启短 `BEGIN IMMEDIATE` 事务，重算任务建议输入哈希、目标标签状态
和看板本体摘要；任一值变化都会返回冲突且零写入。成功路径在一个事务中写入规范
标签/语义/原子、任务绑定、普通任务—标签事件、一个 `bootstrap_label` 根本体操作，
以及对应的新增原子影响。校验摘要会写入根操作的变更快照和 CLI 输出；它不等同于
提交后受信任校验。无可用向量提供程序时，校验会在写入前失败；不需要本地向量校验时，
省略 `--verify` 和 `--vector-config`。

示例：

```bash
kanban label create backend --color blue
kanban label delete old-label --json
kanban label delete old-label --force --json
kanban label semantics delete old-label --expected-semantics-hash sem_abc123 --reason "删除标签身份前停用旧语义" --json
kanban label bootstrap default#12 database --description "数据库持久化工作" --applies-when "涉及 SQLite 迁移" --positive-example "新增数据表迁移" --json
kanban label bootstrap default#12 database --description "数据库持久化工作" --applies-when "涉及 SQLite 迁移" --positive-example "新增数据表迁移" --vector-config .kb/vector.toml --min-verify-score 0.50 --json
kanban label add default#12 backend
kanban label create api
kanban label add default#12 backend api
kanban label add --create-missing default#12 scratch-label --json
kanban label remove t_01HX... backend
kanban label list --json
```

人类可读输出使用紧凑的标签行：

```text
backend l_01HX... color=blue
```

如果任务有人类可读的标签，摘要末尾会追加方括号标签列表：

```text
default#12 [ready] P1 修复 API 回归 [backend,p1] · plan: planned · steps: 0/0
```

`label suggest` 返回任务级标签建议。带内置标签原子向量存储的构建，会把任务标题和
描述的嵌入向量作为查询，使用 `lancedb_label_atoms` 按残差多轮检索正向标签原子，
并用原始查询检索负向原子，以施加惩罚或抑制。求解器在标签组层执行 Group OMP 选择，
再用所选标签的最相关正向原子向量执行非负重拟合；`coverage` / `residual_norm`
来自该原子级拟合向量，
其中 `coverage = clamp(1 - residual_norm, 0.0, 1.0)`，因此二者不是两份独立
证据；`coverage_cosine` 是原始查询与拟合向量的余弦相似度，
可作为独立补充指标。
候选标签只有在试探性重拟合后带来足够的残差范数降幅，才会进入结果；覆盖率或残差范数
达到停止阈值后，求解器会提前停止，而不是凑满 `--max-selected-labels`。候选组与
已选标签语义向量过度相似时会被跳过，以减少重复语义标签同时出现在已选标签中；
这不会合并或删除规范标签。
`needs_new_label` 是兼容字段，只表示存在需要人工审核的标签覆盖率诊断；
具体原因必须读取 `reason_codes`，例如 `no_selected_labels`、
`coverage_below_threshold`、`residual_above_threshold`、`unexplained_residual`，
或与降级相关的原因。不要把 `coverage` 与 `residual_norm` 重复计票，也不要仅凭
`needs_new_label=true` 创建新词汇；必须结合 `reason_codes`、证据原子、诊断信息
和人工语义判断。
它不会自动创建新标签，也不会写入新标签提案。应用建议时仍使用现有
`label add <task_ref> <label>...` / API attach 流程。

默认轻量 CLI 通过向量辅助适配器运行标签向量查询；辅助进程/提供程序不可用时，
命令成功返回降级结果而不是失败，且 `needs_new_label=false`。`--vector-config`
使用与 `kanban vector configure/status` 相同的 TOML 解析规则，并把解析出的嵌入模型
传给辅助进程查询。`LabelAtomHit.distance` 保留 LanceDB `_distance` 的原始语义；
建议/提案分数只根据返回的原子向量与当前查询/残差，在本地计算余弦相似度，
不从距离值推导。

JSON 输出：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "task_id": "t_01HX...",
    "board_id": "b_01HX...",
    "selected_labels": [],
    "candidates": [],
    "coverage": 0.0,
    "coverage_cosine": 0.0,
    "residual_norm": 1.0,
    "needs_new_label": false,
    "reason_codes": ["degraded_result", "vector_store_disabled"],
    "degraded": true,
    "diagnostics": ["vector_store_disabled"]
  }
}
```

人类可读输出简洁列出建议标签、分数、权重和是否已应用；降级时追加诊断信息行。

`--limit` 只控制最终输出中 `selected_labels` / `candidates` 的最大条数，不会收窄
求解器内部搜索能力。内部能力由 `--candidate-limit`、`--atom-limit` 和
`--max-selected-labels` 分别控制：候选标签组数、每轮原子向量检索上限，
以及最多进入非负重拟合的标签数。所有限制参数都必须是
`1..=1000`；`--min-score` 必须在 `0..=1`。

标签本体的长期回归语料库目前是本地测试基础设施，不是会写生产数据库的 CLI 变更流程。
修改标签求解器、语义/原子生成、受信任校验或重要标签本体时，可以运行：

```bash
just test-p kanban-sqlite label_ontology_longitudinal_regression
```

该测试在临时 SQLite 数据库中建立固定的重要标签、已知正样本任务和负对照任务，重建
内存标签原子索引，保存基线 `label suggest` 结果，再模拟一次范围过宽的原子变更，
并比较已选标签、分数和证据原子。它会断言正常语料库运行不会修改 `labels`、
`task_labels`、`label_semantics`、`label_atoms` 或本体账本记录；真实项目语料库
应在积累稳定任务后逐步扩展，但不应成为每个日常任务标签绑定的默认必跑步骤。

`label semantics` 管理当前看板上已有标签的语义字典。`<label>` 接受标签名称或
`l_...` ID。`upsert` 默认是补丁：`--description` 只在提供非空值时覆盖当前描述，
数组参数会追加到对应集合，`--remove-*` 只删除匹配的既有文本；未提供的字段不会被
解释为清空。传入 `--replace` 时才执行完整替换，此时未提供的数组会成为空数组，
并且不能同时传入 `--remove-*`。`--expected-semantics-hash <hash>` 是比较并交换保护：
哈希不等于当前语义哈希时返回冲突且不写入。`--reason` 和 `--source-signal` 会进入
`update_semantics` 本体操作；即使没有来源信号，建设性语义变更也会在同一事务写入
前后哈希、变更快照和操作者来源。`upsert` 会写入 `label_semantics` 并同步重建
该标签的 `label_atoms`，随后将派生的标签原子向量索引标记为脏。数组参数可重复；
空白值去除首尾空白后会被丢弃。生成原子时，有描述的标签会生成一个规范的
`description` 原子：`label: {name}\ndescription: {description}`；没有描述时
才使用 `name` 回退原子。原子文本会进一步规范化空白：折叠每个非空行内部的空白，
保留规范行分隔。同一标签下相同 `polarity + kind + normalized_text` 的原子会去重
并保留首次序号，`id` / `content_hash` 不包含序号，因此只调整数组顺序不会改变
同一文本原子的身份。
`delete` 是受 CAS 保护的语义清空操作：必须传入
`--expected-semantics-hash <hash>` 和非空 `--reason <text>`。它删除该标签的
语义与 SQLite 原子，但不删除规范标签身份或任务—标签绑定；同一事务会写一个
`update_semantics` 根本体操作，操作后快照为空，并为实际移除的原子写入 `removed`
原子影响，随后将标签原子索引标记为脏。哈希不匹配时，规范数据、操作、影响和脏状态
全部不变。成功返回 `{ "data": { "deleted": true } }`。需要在清空后删除标签身份时，
先清空语义，再执行 `label delete`。

`label atoms list` 读取 SQLite `label_atoms` 物化投影。这些原子来自
`label semantics upsert`、`label bootstrap`、`label ontology apply atom`，或接受标签
提案后生成的语义；它们是 `lancedb_label_atoms` 派生索引的输入，不是派生索引本身。

`label atom explain <atom-id-or-content-hash>` 是 `label atoms explain` 的单数别名，
按当前看板的原子 ID 或稳定 `content_hash` 解析现有原子，并返回当前原子、规范语义、
来源操作、支持信号/来源任务和校验历史。当前原子存在，但没有本体来源操作引用其 ID 或
内容哈希时，命令成功返回 `legacy_untracked=true` 和 `legacy_reason`；未知 ID/哈希
返回未找到。JSON 输出是 `LabelAtomExplainRecord`，包含 `query`、`atom`、
`current_semantics`、`provenance_actions`、`supporting_signals`、
`validation_history`、`legacy_untracked` 和 `legacy_reason`。由于内容哈希不含序号，
语义重建后同语义原子的 ID 改变时，仍可用内容哈希解释历史。

`label atom-index status` 返回标签原子向量索引的状态。未配置提供程序或辅助进程不可用时，
仍成功返回禁用/降级状态。JSON 保留兼容字段 `message`，并返回结构化的
`diagnostics: string[]`、`dirty: boolean | null`、`board_dirty: boolean | null`；
调用方应使用结构化字段判断脏状态/错误，而不要解析 `message` 文案。`status` 通过
辅助进程的 `label-atoms-status` 命令读取 `LANCEDB_LABEL_ATOMS_STORE` 与
`label_atom_index_boards` 语义；`query` 通过辅助适配器查询标签原子向量索引，
`--polarity` 只接受 `positive` 或 `negative`，人类可读输出和 JSON 命中记录都把
LanceDB `_distance` 暴露为 `distance`。`rebuild` 通过辅助进程的
`rebuild-label-atoms` 命令重建标签原子派生索引；辅助进程/提供程序不可用时返回
显式错误，不修改 SQLite 中的规范标签事实，也不把分块存储标记为成功。

`kanban vector query-label-atoms` 是公开的原始辅助查询入口，支持文本查询和原始向量查询。
输入必须且只能选择一种：位置参数 `<text>`、`--text-file <PATH|->`、
`--vector-json <JSON>` 或 `--vector-json-file <PATH|->`。`-` 表示从标准输入读取。示例：
`kanban vector query-label-atoms --text-file query.txt [--polarity positive|negative] [--limit N] [--embedding-model MODEL] [--vector-config <toml>]`，或
`kanban vector query-label-atoms --vector-json-file vector.json [--include-vector] [--embedding-model MODEL] [--polarity positive|negative] [--limit N]`。
`--include-vector` 只对辅助进程支持的原始向量/向量命中输出有意义。

`label propose` 是独立的新标签语义提案流程，不复用或改变 `label suggest`。
它先读取当前任务级标签建议的 `coverage` / `coverage_cosine` / `residual_norm` /
最相关现有标签。没有 `--proposal-json` 时，默认提供程序不可用；命令成功返回降级尝试，
不创建规范标签、`label_semantics`、`label_atoms` 或 `task_labels`。日常标签建议
不依赖该提案提供程序。
`--limit` 只截断提案尝试中复用的建议输出；`--candidate-limit`、`--atom-limit`、
`--max-selected-labels`、`--min-score` 会在提案持久化前调节底层标签建议求解器，
用于计算覆盖率、覆盖率余弦值、残差范数和最相关现有标签。
`--vector-config` 使用与 `label suggest` 相同的 TOML 解析规则。默认轻量 CLI
通过向量辅助适配器运行残差校验；未配置或辅助进程/提供程序不可用时保持降级回退，
不写入普通标签或任务—标签关联。

提供程序边界：CLI 当前只使用禁用的提供程序，或通过 `--proposal-json` 显式传入的
本地/离线候选。真实 LLM 提供程序不属于 `kanban-sqlite`；未来若接入本机 AI 运行时，
应在 CLI、本地运行时或独立 AI crate 中实现 `LabelProposalProvider` 适配器，
再把候选交给 SQLite 服务做确定性校验和持久化。

`--proposal-json` 提供本地/离线提供程序输出：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "name": "database",
  "description": "数据库持久化工作",
  "applies_when": ["涉及 SQLite 迁移"],
  "excludes_when": ["只调整界面"],
  "positive_examples": ["新增数据表迁移"],
  "negative_examples": ["只修改 CSS"]
}
```

数组字段缺省时按空数组处理。`name` 不能为空，且描述或任一语义数组至少需要提供
一个非空值。只有当前启发式覆盖率不足时才持久化提案。与现有标签发生规范化名称冲突的
候选会写成 `rejected` 提案，并在诊断信息中返回 `near_duplicate_label_conflict`；
该规范化名称检查忽略大小写、空白和标点，是确定性的近重复启发式规则。
覆盖率不足的候选还会执行残差最相关结果加间隔校验：候选语义的残差分数和现有标签
最相关结果，都按返回的原子向量在本地计算余弦相似度，不从 LanceDB 距离推导；
候选必须超过现有标签最相关结果，且超过幅度达到固定间隔。校验失败时，本次尝试仍会
把候选持久化为 `rejected` 提案，诊断信息包含
`label_proposal_residual_top1_failed` 或
`label_proposal_residual_margin_insufficient`，用于审计为什么没有进入可接受状态。
如果残差校验不可用或已降级，且没有明确通过最相关结果加间隔校验，本次尝试返回
`degraded=true`、`proposal=null`，不新增提案记录，也不创建规范标签、
`label_semantics`、`label_atoms` 或 `task_labels`；诊断信息包含
`label_proposal_residual_validation_unavailable` 和具体原因。
传入 `--source-signal <los_...>` 时，提案创建成功后会在同一事务写入
`create_label_proposal` 本体操作，并通过操作—信号链接记录该提案由哪些已确认的
词汇缺口信号支持；提案记录与来源操作要么同时写入，要么一起回滚。来源信号默认必须是
同一看板上 `confirmed` 的 `vocabulary_gap` + `bootstrap_label` 信号，且规范化后的
`proposed_label_name` 必须等于提案名称。`--actor-type` / `--agent-type` 控制该
`create_label_proposal` 操作的操作者来源；操作者名称仍来自全局 `--actor`。
确实需要把已确认的同看板来源信号重定向到该提案时，必须同时传入 `--allow-retarget`
和非空 `--retarget-reason <text>`；原因和来源信号原始目标/候选标签会写入
`change_json.retarget_override`。重定向不会放宽看板/状态要求。

`label proposals accept` 只接受 `proposed` 提案。接受操作与单任务引导共用
同一个采用原语：创建规范标签、`label_semantics` 与 `label_atoms`，
将标签原子索引标记为脏，并写入 `bootstrap_label` 本体操作；提案记录、规范写入和
操作来源要么在同一事务中成功，要么一起回滚。它不会自动给来源任务写入
`task_labels`。未传 `--source-signal` 时仍会记录引导操作，只是没有操作—信号链接；
传入 `--source-signal <los_...>` 时会通过链接记录该新标签引导的信号来源，
且这些来源信号必须是同一看板上的 `confirmed` 信号。`--actor-type` /
`--agent-type` 控制该 `bootstrap_label` 操作的操作者来源；操作者名称仍来自
全局 `--actor`。
默认是 `user`。`--actor-type agent` 必须提供非空 `--agent-type`；`user` 不能提供
`--agent-type`。来源信号默认还必须是 `vocabulary_gap` + `bootstrap_label`，
且规范化后的 `proposed_label_name` 必须等于提案名称。如果提案已有
`create_label_proposal` 操作，接受时产生的 `bootstrap_label` 操作会把
`parent_action_id` 指向该创建操作，形成“提案创建 → 引导接受”链路。
确实需要把已确认的同看板来源信号重定向到该提案时，必须同时传入
`--allow-retarget` 和非空 `--retarget-reason <text>`；该原因、来源信号的原始
目标/候选标签和最终提案/结果标签会写入引导操作的
`change_json.retarget_override`。重定向不会放宽看板/状态要求。
`label proposals reject` 把提案标记为 `rejected`，不接受 `--source-signal`。
已接受或已拒绝的提案不能再次决策。

`label ontology record` 记录一次标签判断观察，并写入其中的子信号。
推荐输入边界是：工具采集或接收未经改写的 `label suggest` 快照，服务从快照派生
覆盖率、残差、降级、诊断等观察指标；智能体只提交候选、最终判断、信号、候选原子和
理由。CLI 可以用 `--capture-suggest` 在记录前用同一组建议选项运行一次真实的
`label suggest`，也可以用 `--suggestion-snapshot <path|->` 读取已保存的原始建议
JSON。快照可以是直接的建议响应，也可以是带 `data` 封装的 JSON 响应。

`--input` 只接受契约所有的自然 JSON 结构；旧 `_json` 兼容同级字段（例如
`diagnostics_json`、`related_labels_json`）会作为未知字段拒绝。新调用方不应重复手写
`suggest_coverage`、`suggest_residual_norm` 或 `diagnostics`。如果快照中已有这些字段，
而输入又提供冲突的标量或诊断信息，命令会失败。服务会读取当前任务快照、解析目标标签
引用、计算规范化候选标签名称、信号键和候选原子内容哈希；观察记录同时保存用于完整审计的
`task_snapshot_json.content_hash`，以及只基于标签建议输入（规范化标题和描述）的
`suggest_input_hash`。它只写账本，不修改 `task_labels`、`label_semantics`、
`label_atoms`、标签原子索引或提案。

信号输入会在写入前做本体契约校验。`candidate_atom` 的
`applies_when` / `positive_example` 只能使用 `positive` 极性，
`excludes_when` / `negative_example` 只能使用 `negative` 极性。
`add_positive_atom` 必须提供目标标签和正向候选原子；
`add_negative_atom` 必须提供目标标签和负向候选原子；
`update_semantics` 必须提供目标标签；`bootstrap_label` 必须提供
`proposed_label_name`；`rename_label` 必须提供目标标签和
`proposed_label_name`；`split_label` / `merge_labels` 必须提供目标标签和非空
`related_labels`。观察指标 `suggest_coverage`、`suggest_coverage_cosine`、
`suggest_residual_norm` 以及信号指标
`suggest_score` / `confidence` 必须是 `0.0..=1.0` 范围内的有限数；`suggest_rank` 必须为
`null` 或 `>= 1`。
`rename_label` / `split_label` / `merge_labels` 当前只作为审核信号的候选操作保存，
CLI 不提供写入规范结构变更操作或结构计划操作的命令；旧结构计划记录只读展示为
不支持的校验要求。

使用已保存标签建议快照时，推荐的输入结构如下：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "actor": {"name": "label-agent", "type": "agent", "agent_type": "local"},
  "agent_candidates": [
    {"label": "cli", "reason": "该任务会改变 CLI 行为。"}
  ],
  "final_decision": {
    "selected": ["cli"],
    "rejected": []
  },
  "signals": [
    {
      "kind": "false_negative",
      "target_label_ref": "cli",
      "related_labels": [],
      "proposed_action": "add_positive_atom",
      "candidate_atom": {
        "polarity": "positive",
        "kind": "applies_when",
        "text": "扩展 CLI 子命令、命令参数、帮助输出或机器可读 JSON 行为"
      },
      "proposal": {},
      "agent_selected": true,
      "suggest_state": "candidate",
      "suggest_score": 0.08,
      "suggest_rank": 6,
      "final_selected": true,
      "rationale": "该任务扩展了 CLI 接口。"
    }
  ]
}
```

调用示例：

```bash
kanban label suggest default#42 --json > /tmp/default-42-suggest.json
kanban label ontology record default#42 \
  --input /tmp/default-42-record.json \
  --suggestion-snapshot /tmp/default-42-suggest.json \
  --json
```

或者让 CLI 在记录前采集快照：

```bash
kanban label ontology record default#42 \
  --input /tmp/default-42-record.json \
  --capture-suggest \
  --vector-config ./vector.toml \
  --json
```

`label ontology list` 默认只返回 `open` 和 `confirmed` 信号。`--include-all`
返回完整历史；`--status`、`--kind` 可重复过滤，`--task`、`--label` 和
`--proposed-label` 用于按来源任务、目标标签或候选新标签查询。
`label ontology show` 返回信号、观察和关联操作。`label ontology review`
是只读聚合审核队列视图，默认只聚合 `open` 和 `confirmed` 信号；传入
`--include-all` 时包含 `resolved` / `rejected` / `superseded` 历史。`--group-by`
支持按 `label`、`candidate-atom`、`proposed-label` 或需显式选择的 `cluster` 聚合，
`--limit` 限制返回组数。`--json` 中每个组返回聚合维度、键、相关标签/候选原子/
候选标签、聚类键/原因（仅聚类视图有值）、不同任务数、信号/状态/降级/操作数量、
分数摘要、任务引用示例、信号 ID、操作 ID 和提案 ID。排序优先使用不同任务数，
其次是已确认数量、最新信号时间和键。

审核组只表示一组信号共享同一个聚合键，不证明它们一定来自同一个根因。
`--group-by label` 使用 `target_label_id` 作为键，缺失目标标签时使用
`no-target-label`。`--group-by proposed-label` 使用规范化后的候选标签名称，
缺失候选新标签时使用 `no-proposed-label`。`--group-by candidate-atom` 优先使用
`candidate_content_hash`；如果信号没有候选原子，则键会包含信号类型、
目标标签或候选标签，以及候选操作，例如
`no-candidate-atom|kind:vocabulary_gap|proposed:ontology ledger|action:bootstrap_label`。
这个回退规则避免把不同类型、不同标签或不同候选操作的空候选信号合并到一个全局桶。
`--group-by cluster` 是只读审核辅助：它不写规范原子，也不会确认、应用、校验或关闭
信号。聚类键在每次查询时从已有信号文本重建，优先使用词法规范化后的候选文本，
其次是候选标签，再其次是理由，最后才回退到类型/操作/目标/候选标签作用域组合；
所有聚类键都带有信号类型、候选操作、目标标签和候选标签作用域，避免跨标签、操作或
边界误合并；`cluster_reason` 说明当前键的来源。

`task_count` 是组内不同来源任务数，也是默认热度排序的第一依据；同一任务上的多条信号
仍只贡献一个不同任务。`signal_count` 是原始信号记录数，用于判断一组里有多少审查项；
它没有分母，不能解释为模型错误率、精确率或召回率。`degraded_count`、状态数量、
分数摘要和任务引用示例只是审核人员的排查线索。排序为 `task_count` 降序、
`confirmed_count` 降序、`latest_signal_at` 降序、`key` 升序；需要判断是否同一问题时，
应继续查看组内任务示例、信号 ID 和 `label ontology show` 详情。

`label ontology quality` 是只读质量/分析报告。它从当前看板的
`label_ontology_observations` 取得可审计分母，并从 `label_ontology_signals`
取得原始分歧数量；不会写入任务、标签、语义、原子或账本操作。JSON 输出包含：

- `denominator.source="label_ontology_observations"`、`observation_count`、
  `distinct_task_count`、一致/降级观察数量、时间范围和
  `sample_task_refs`。
- `disagreement.signal_count`、`disagreement.distinct_task_count`、`by_kind`、
  `by_status`。
- `rates.disagreement_task_rate`：只在分母至少包含一个一致观察时返回；
  只有信号的历史不会输出伪错误率。
- `precision_recall.available=false`，直到项目有带预期标签的独立评估样本群。
  原始信号只能说明记录过分歧，不能单独证明精确率、召回率、漏报率或模型错误率。

生命周期命令写入操作并同步更新信号状态：

- `confirm`：`open` 信号进入 `confirmed`。
- `reject`：把信号标记为 `rejected`。
- `supersede --by`：把重复或过时信号标记为 `superseded`；写入前会沿替代项的
  `superseded_by_signal_id` 链检查，拒绝会回到任一来源信号的环。
- `resolve --no-change`：记录无需修改本体的解决结果。

这些生命周期命令只记录审核/状态变化，不接受规范变更来源字段。
`add_positive_atom`、`add_negative_atom`、`adopt_existing_atom`、
`update_semantics`、`create_label_proposal`、`bootstrap_label`、`revert_ontology_mutation`
和 `validate` 等操作记录，只能由
`label semantics upsert`、`label ontology apply atom`、`label propose`、
提案接受、`label bootstrap`、`label ontology revert`、`label ontology validate`
等专用命令/服务路径在同一
同一事务中写入。通用操作命令不能伪造规范数据前后哈希、结果原子/标签/提案或校验载荷。
生命周期、应用原子、校验和带 `--source-signal` 的提案接受操作都支持
`--actor-type user|agent` 与 `--agent-type <type>`。这些标志只控制本体操作记录的
`created_by_type` / `agent_type`；操作名称仍来自全局 `--actor`。默认为
`--actor-type user` 且不写 `agent_type`。`agent` 操作者必须提供非空
`--agent-type`，`user` 操作者带 `--agent-type` 会被拒绝。

`label ontology apply atom` 只接受 `confirmed` 来源信号。它会读取目标标签的当前语义，
把泛化文本加入对应数组，走现有的语义更新/原子重建路径。如果规范内容实际新增原子，
会写入 `add_positive_atom` 或 `add_negative_atom` 操作，记录生成原子的软引用、
内容哈希、前后哈希、单份变更快照和一个 `added` 原子影响，并把
`validation_requirement` 置为 `required`。如果同内容原子已经存在，则写入仅记录来源的
`adopt_existing_atom` 操作，记录现有原子软引用、相同的前后哈希和来源信号链接；
该操作不修改语义/原子，不把原子索引标记为脏，`validation_requirement=none`
且有效结果为 `not_required`。
默认要求所有带 `target_label_id` 的来源信号都指向被修改标签；不匹配时拒绝并列出
违规信号 ID。原子文本不需要逐字等于来源信号的候选文本，审核人员可以写更泛化的规范
原子。确实需要重定向已确认的同看板信号时，必须传入 `--allow-retarget` 和非空
`--retarget-reason <text>`；操作的 `change_json.retarget_override` 会记录原因、
来源信号的原始目标/候选标签和最终目标标签。重定向不会放宽看板/状态要求。
该命令只有在规范原子实际新增时才把标签原子索引标记为脏；向量索引重建和后续建议校验
仍是第二阶段。

`label ontology revert <action_id>` 为已提交的标签级规范本体变更追加
`revert_ontology_mutation` 操作，并把目标标签语义恢复到被撤销操作的
`canonical_before_hash` / `change_json.before` 快照。当前只支持
`add_positive_atom`、`add_negative_atom` 和 `update_semantics`；不处理引导操作的
标签身份或任务绑定回滚。为避免覆盖后续修改，命令要求当前规范语义哈希仍等于目标操作的
`canonical_after_hash`；传入 `--expected-current-hash <hash>` 时，还会先对调用方持有的
快照执行 CAS 检查。成功后会写入仅追加的撤销操作，`parent_action_id` 指向被撤销操作，
复制原操作的来源信号链接，记录撤销前后快照，为本次撤销实际新增/移除的原子写入原子
影响，把标签原子索引标记为脏，并把 `validation_requirement` 置为 `unsupported`。
原变更操作不会被修改或删除。

所有规范语义/原子变更事务都遵循单根操作合同：同一事务只写一条根变更操作，
`change_json` 只保存一次语义前后快照；实际新增或删除的原子通过
`label_ontology_action_atom_effects` 记录 `added` / `removed` 影响。只修改描述的
补丁会写一条根操作和零个原子影响；空操作补丁不写操作/影响，也不把索引标记为脏。
原子解释优先使用影响记录；旧版逐原子操作仍保持兼容读取。

`label ontology validate` 为一个变更操作追加 `validate` 操作。父操作必须是同一看板上
`validation_requirement=required` 的规范变更操作，并携带规范结果证据（例如原子、
结果标签/提案引用、规范哈希和非空变更快照）。父操作的 `validation_status` 是历史兼容
字段，不再单独表达“是否需要验证”；读取时通过归并器暴露有效结果：
`not_required|unsupported|pending|passed|failed|partial`。

普通 `--input` 路径属于外部证明：CLI 读取调用方提供的 JSON，服务只把提供的载荷、
来源信号用例摘要、任务快照/建议输入哈希对比和父操作结果引用包装进校验封装。
公共的提供/采集载荷只在顶层 `manual` 保存一次；生成的 `cases[]` 使用
`after.manual_case_ref` 引用 `manual.cases[]` 中对应信号的证据，不在每个用例中重复
整份载荷。该路径可记录 `failed` / `partial` 诊断，但不能把 `passed` 写成受信任证明；
即使 JSON 自称
`evidence_type="automated"`，`--status passed` 也会被拒绝，关联信号不会被
关闭。

`--trusted` 路径才是受信任的自动校验。它不接受 `--input`，也不接受调用方手写的受信任
证据 JSON；CLI 只能走内置采集器。“受信任”表示工具在当前父操作、来源信号、规范哈希、
原子索引代次和指定用例/对照上做了机械采集和检查，不表示本体在全局语义上正确。
CLI 必须有可用的标签原子向量工作流适配器（当前轻量 CLI 尚未接入；旧内置
`vector-lancedb` 构建需可解析 `--vector-config` 或默认配置），先在 SQLite 事务外
重建原子索引，再用同一
`--limit` / `--candidate-limit` / `--atom-limit` / `--max-selected-labels` /
`--min-score` 选项对关联来源信号重新运行 `label suggest`，由工具生成
`evidence_type="trusted_automated"`、`collector.source="label_ontology_validate_trusted"`、
`embedding_model`、`solver_options`、干净的 `index.status` / `index.generation`
和逐信号 `cases[]`。写操作时，服务会在短事务内重新核验父操作、来源信号、规范结果哈希、
原子索引脏状态/错误状态和代次，防止查询后规范或派生状态已变化。脏、错误、禁用的索引，
缺失代次或过期代次，都不能产生受信任的通过结果。

`--positive-control <TASK_REF>` 与 `--positive-control-waiver <REASON>` 只用于
负向原子的受信任校验，且二者互斥；非负向父操作携带这些参数会被拒绝。
豁免只能由 `--actor-type user` 提交，原因必须非空。负向原子父操作若两者都缺失，
会在采集前失败。

`cases[]` 的 `case_type` 必须匹配父操作：`positive_atom`、`negative_atom`
或 `bootstrap_label`。正向原子校验要求 `after.degraded=false`、结果原子 ID/内容哈希
出现在 `after.evidence_atoms[]`、目标标签被选中或分数不低于 0.50，且分数/覆盖率
不恶化。负向原子校验要求结果原子 ID/内容哈希出现在
`after.negative_evidence_atoms[]`；在误报任务上，必须证明
`after.target.selected=false`，或前后分数都存在且结果分数低于先前分数；并且必须提供
至少一个 `after.positive_controls[]` 且全部通过、未退化，或提供原因非空的
`after.positive_control_waiver`。引导标签校验要求所有关联来源信号都有通过用例，
新标签/结果标签被选中或分数不低于 0.50，且证据原子来自结果标签。

校验可比性默认使用观察记录的 `suggest_input_hash`；状态、`updated_at`、
`lock_version` 或任务标签绑定只改变完整快照时，写入 `task_metadata_drift` /
`label_binding_drift` 警告，不会让已通过校验过期。标题/描述变化会写入
`suggest_input_drift` 并使该用例不可比较；旧观察缺少 `suggest_input_hash` 时写入
`legacy_suggest_input_hash_missing`，不能静默通过。`--status passed` 会把关联来源
信号转为 `resolved`；`failed` / `partial` 保留历史和证据，来源信号继续等待后续修正
或人工处理。

`label propose --json` 返回结构化尝试结果：

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

---

## 9. 评论命令

```bash
kanban comment add <task_ref> (<body>|--body-file <PATH|->) [--kind note|decision] [--author-type user|agent] [--agent-type <type>] [--metadata-json <json>|--metadata-json-file <PATH|->]
kanban comment list <task_ref>
```

`--actor` 提供评论作者的显示身份。省略 `--kind` 时，服务默认为 `note`。
省略 `--author-type` 时，服务默认为 `user`；Codex、现有实验性调度器或其他自动写入方
应传入 `--author-type agent --agent-type <type>`。`signal` 是持久化评论类型，
但用户应通过 `kanban signal record` 创建信号反向链接评论，而不是手动使用
`comment add --kind signal`；这样信号账本和反向链接评论会在同一事务中写入。
`--body-file <PATH|->` 从文件或标准输入读取较长评论正文，并与行内 `<body>` 互斥；
多行或对 shell 敏感的评论文本推荐使用这种方式。`--metadata-json` 默认为 `{}`，
并且必须是 JSON 对象；`--metadata-json-file <PATH|->` 从文件或标准输入读取相同的
JSON 载荷，避免结构化载荷的 shell 引号问题，并与 `--metadata-json` 互斥。
使用 `--kind decision` 时，元数据必须满足结构化决策模式：非空 `options`、
唯一的小写 ASCII 选项 `slug`、与某个 slug 匹配的 `selected`、非空 `reason`，
以及可选但非空的 `risk` / `verification`。

智能体命令失败记录应保存为评论，而不是只留在聊天记录中。使用
`comment add --author-type agent --agent-type <name> --kind note --metadata-json <json>`，
在人类可读正文中写简短摘要，并把结构化记录放入元数据。最小记录载荷是包含以下字段的对象：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "tool": "kanban-cli",
  "command": "kanban task step add",
  "argv": ["kanban", "task", "step", "add", "..."],
  "intent": "添加必需的执行计划步骤",
  "why_selected": "任务需要跟踪执行计划，因此智能体选择了步骤命令",
  "actual_error": "unexpected argument 'true' found",
  "repair": "改用规范的独立 --required，或受支持的 --required true/false 形式重试",
  "product_signal": "面向智能体的布尔标志兼容性缺口",
  "followup_task": "default#123"
}
```

调用方可以添加其他字段，但对于把智能体命令失败记录转化为解析器、文档、技能或测试工作的
工具而言，这些字段名是稳定的最小契约。

面向智能体的富文本输入示例：

```bash
kanban comment add default#12 --body-file - <<'EOF'
正文可以安全包含 $VAR、$(command)、`code`、JSON 和多行文本。
EOF
```

有意义的多选项决策应使用 `--kind decision`。正文保留为人类可读的回退摘要，
结构化选项和选择数据只放在 `--metadata-json` 中：

```text
已决定继续使用 comment metadata 承载结构化决策信息，正文保留为简短结论，方便没有结构化渲染的环境阅读。
```

决策元数据示例：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "options": [
    {
      "slug": "comment-metadata",
      "title": "使用评论元数据",
      "detail": "把结构化决策数据存入 task_comments.metadata_json。"
    },
    {
      "slug": "decision-table",
      "title": "创建决策表",
      "detail": "创建独立的 task_decisions 表和选项记录。"
    }
  ],
  "selected": "comment-metadata",
  "reason": "让决策紧邻任务讨论，避免产生并行时间线。",
  "risk": "元数据模式需要严格校验。",
  "verification": "CLI、API 和桌面端测试覆盖创建、读取、渲染及非法元数据拒绝。"
}
```

琐碎命名、格式调整或纯机械选择无需创建决策评论。

人类可读输出保持紧凑，包含评论 ID、任务 ID、`created_at`、类型、作者身份、
`author_type`、可选的 `agent_type` 和正文：

```text
c_01HX... task=t_01HX... created_at=1717520000000 [note] 用户甲 (user): 可以审核了
c_01HX... task=t_01HX... created_at=1717520000100 [note] codex (agent/root): 测试已通过
```

JSON 输出使用标准封装：`add` 返回契约评论 DTO，`list` 返回该 DTO 列表，并包含自然、
无损的 `metadata` 对象。输入标志名 `--metadata-json` / `--metadata-json-file` 保持不变。
创建评论会写入 `task_events(kind='task.comment.created')`。

---

## 10. 事件命令

```bash
kanban events <task_ref>
kanban events --board default
```

不传 `<task_ref>` 时，按当前看板列出事件。已归档看板的事件仍可通过显式 `--board` 读取。

---

## 11. 运行记录命令

```bash
kanban runs <task_ref>
kanban run show <run_id>
kanban run logs <run_id>
kanban run logs <run_id> --tail-bytes 65536
```

`kanban run logs` 默认最多读取 256 KiB。传入 `--tail-bytes` 时只返回日志末尾的指定
字节数。`task_runs.log_path` 必须解析到受信任日志目录，且文件名匹配 `<run_id>.log`；
可疑路径会被拒绝。

---

## 12. 服务端命令

`kanban serve` 是受支持的本地 API 入口：

```bash
kanban serve
kanban serve --host 127.0.0.1 --port 8721
kanban serve --quiet
kanban serve --log-level warn
kanban serve --search-sync-interval-ms 5000
```

默认地址是 `127.0.0.1:8721`；`--host` 必须解析为回环地址，`--port` 指定端口。
仓库仍保留实验性 `kanban dispatch` 代码，但它不属于公开支持路径，本规范不把其内部
参数作为用户契约。

`kanban serve` 默认把启动诊断、HTTP 请求记录和优雅关闭通知写入标准错误；标准输出
保留给显式机器可读输出，不用于服务日志。使用 `--quiet` 可抑制服务诊断，
`--log-level <off|error|warn|info|debug|trace>` 可简单覆盖详细程度；也可省略两者并设置
`RUST_LOG`，使用高级跟踪过滤器。默认过滤器是
`kanban=info,kanban_cli=info,kanban_server=info,tower_http=info,kanban_desktop=info`。

Ctrl-C/SIGINT 会触发 `kanban serve` 优雅关闭、释放运行锁、以 `0` 退出，且不写标准输出。
`--quiet` 和 `--log-level off` 会抑制优雅关闭通知。关闭期间第二次按下 Ctrl-C 会立即
以代码 `130` 退出。

使用 `tantivy-backend` 构建二进制时，`kanban serve` 会启动保守的后台搜索同步循环。
循环在启动时立即尝试一次，随后每隔 `--search-sync-interval-ms` 毫秒调用
`sync_search_index`（默认 `5000`）。使用 `--search-sync-interval-ms 0` 可禁用。
未启用 `tantivy-backend` 时，该标志仍会被接受，但不会启动后台索引任务。

---

## 13. 搜索命令

### 13.1 `kanban search`

```bash
kanban search <query> [--status ready] [--status review] [--assignee worker-a] [--label backend] [--include-archived] [--limit 20] [--offset 0] [--json]
```

默认 CLI 构建启用 `tantivy-backend`。当 `index/v1/tasks/` 存在可读 Tantivy 索引时，
`kanban search` 使用 Tantivy；索引缺失、损坏、过期，或二进制显式以
`--no-default-features` 构建时，会回退到 SQLite，并在顶层 `meta` 中标记过期。
搜索匹配任务标题、描述、评论、运行摘要/错误，以及事件类型/载荷。

`--label <name-or-id>` 可重复；多个标签使用 AND 语义，并在搜索分页前过滤任务。
带标签过滤的 Tantivy 搜索会回退到 SQLite，以保持当前标签关联关系和分页语义正确。

形似任务引用的查询始终使用 SQLite 精确匹配语义，即使当前存在可用 Tantivy 索引：
纯数字 `12`、`#12` 匹配请求看板内的序号；`board#12` / `board/#12`
只在显式看板与请求看板相同时匹配；`t_...` 只匹配请求看板内的任务 ID。
这些查询不会因为标题、描述或聚合搜索文本包含相同数字/引用片段而返回额外任务。

人类可读输出会紧凑展示公开任务引用、状态、分数、标题，以及可用时的摘要片段。
默认不包含内部 `t_...` 任务 ID；任务 ID 仍可在 JSON 输出和面向诊断/详情的接口中取得。

```text
agent-work#12 [ready] score=60.0 实现状态机 - 就绪规格片段
```

JSON 输出：

<!-- schema-doc-ignore: illustrative or partial payload; committed schema fixtures remain executable authority -->
```json
{
  "data": {
    "hits": [
      {
        "task_id": "t_01HX...",
        "seq": 12,
        "score": 60.0,
        "snippet": "就绪规格片段",
        "task": {
          "id": "t_01HX...",
          "seq": 12,
          "status": "ready",
          "title": "实现状态机"
        }
      }
    ]
  },
  "meta": {
    "backend": "sqlite",
    "stale": false,
    "index_version": null,
    "last_event_id": 42,
    "index_lag_events": 0
  }
}
```

### 13.2 `kanban index`

```bash
kanban index status
kanban index doctor
kanban index rebuild
kanban index sync
```

默认 CLI 构建启用 `tantivy-backend`，Tantivy 索引是可重建的派生缓存；显式以
`--no-default-features` 构建时保留 SQLite 回退：

- `status` 返回后端和元数据。
- `doctor` 为脚本返回同样的回退健康状态元数据。
- `rebuild` 在 SQLite 数据库旁构建或替换 `index/v1/tasks/`，并在 `app_settings`
  中保存干净的高水位状态。
- `sync` 消费已保存高水位之后的 `task_events.id`，删除并重新索引受影响的任务聚合，
  只有成功提交后才推进高水位。
- 任务变更不会在事务内更新 Tantivy；变更后运行 `kanban index sync`，本地服务端/
  桌面会话也可依赖 `kanban serve` 后台同步，或用 `kanban index rebuild` 替换派生索引。

持久化设置键按看板区分，格式为 `search.tasks.state.<board_id>`。其 JSON 包含
`schema_version`、`index_version`、`backend`、`index_name`、`board_id`、
`last_event_id`、`dirty`、`updated_at` 和可选 `message`；现有 `app_settings`
处理会将它纳入 JSONL 导出/导入。

JSON 数据结构：

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

启用 Tantivy 并完成重建后，`backend` 为 `tantivy`，`derived_index` 为 `true`，
`index_version` 为 `tasks-v1`。当前 `MAX(task_events.id)` 大于已保存的
`last_event_id` 时，`stale=true`，`index_lag_events` 报告事件滞后量。索引过期时
搜索会回退到 SQLite，以保证当前结果正确。后台同步错误不会让搜索放行过期的 Tantivy
结果；派生索引落后或不可用时，下一次搜索仍会报告过期/回退元数据，并返回当前 SQLite
结果。

---

## 14. 信号账本

```bash
kanban signal record --board <slug> --input <path|-> --json
kanban signal list --board <slug> [--status open|confirmed|rejected|superseded|resolved]... [--kind <kind>]... [--task <task-ref>] [--include-all] [--limit 100] --json
kanban signal show --board <slug> <signal-id> --json
kanban signal review --board <slug> [--status open|confirmed|rejected|superseded|resolved]... [--kind <kind>]... [--task <task-ref>] [--include-all] [--limit 100] --json
kanban signal confirm [--board <slug>] <signal-id>... (--reason <reason>|--reason-file <PATH|->) [--json]
kanban signal reject [--board <slug>] <signal-id>... (--reason <reason>|--reason-file <PATH|->) [--json]
kanban signal resolve [--board <slug>] <signal-id>... (--reason <reason>|--reason-file <PATH|->) [--json]
kanban signal supersede [--board <slug>] <signal-id>... --by <replacement-signal-id> (--reason <reason>|--reason-file <PATH|->) [--json]
```

`signal list` 和 `signal review` 共享 `status`、`kind`、`task`、`include-all`、
`limit` 查询过滤参数。没有显式 `--status` 时，两者默认只返回 `open` 和
`confirmed`；此时传 `--include-all` 会取消默认状态过滤并返回完整历史。显式
`--status` 始终优先，即使同时传 `--include-all`，结果仍只包含指定状态。
`--status` 和 `--kind` 都可以重复传入。

`record` 输入 JSON 支持 `kind`、`title`、`summary`、`severity`，可选的 `task_ref` /
`task_id` / `run_id` / `comment_id`，以及 `actor`、`agent_type`、`dedupe_key`、
`source`、`evidence` 和可选 `comment.body`。`source` 是标识观察来源的字符串；
`command`、`cwd`、`exit_code`、`stderr` 或相关日志等结构化命令细节应放入自然的
`evidence` 对象。信号响应使用同样的自然对象，而不是转义后的 `evidence_json`
字符串。有任务上下文时，服务会在同一 SQLite 事务中写入信号账本记录和一条
`comment.kind = "signal"` 反向链接。信号反向链接的 `metadata` 包含
`type:"signal_link"`、`signal_id`、`observation_id`、`signal_kind` 和
`signal_status`；通用信号评论元数据保持开放且无损。V1 不会自动创建后续任务。

生命周期转换是 `open -> confirmed|rejected|superseded|resolved` 和
`confirmed -> resolved`。`supersede` 要求替代信号来自同一看板，并拒绝环。
生命周期原因可用 `--reason-file <PATH|->` 从文件或标准输入读取，并与行内
`--reason` 互斥。

## 15. 维护命令

```bash
kanban doctor
kanban stats
kanban backup --out backup.sqlite
kanban export --format jsonl --out board.jsonl
kanban export --format jsonl --out -
kanban import --input board.jsonl --dry-run
kanban import --input board.jsonl --replace
kanban vacuum
kanban checkpoint

kanban entity list [--kind task] [--limit 50]
kanban entity show kb://task/t_...
kanban outbox list [--status pending] [--limit 50]
kanban derived status
kanban graph status
kanban graph rebuild
kanban graph sync
kanban graph neighbors kb://task/t_... [--predicate depends_on] [--limit 50]
kanban graph query (<SPARQL>|--sparql-file <PATH|->) [--limit 50]
kanban vector configure [--provider ollama] [--endpoint http://127.0.0.1:11434] [--model qwen3-embedding:0.6b] [--dimensions 1024] [--skip-check] [--vector-config <toml>]
kanban vector status [--vector-config <toml>]
kanban vector rebuild [--vector-config <toml>]
kanban vector sync [--vector-config <toml>]
kanban vector query-chunks <text> [--limit 10] [--vector-config <toml>]
kanban vector query-label-atoms (<text>|--text-file <PATH|->|--vector-json <json>|--vector-json-file <PATH|->) [--board-id <id>] [--embedding-model <model>] [--polarity positive|negative] [--include-vector] [--limit 10] [--vector-config <toml>]
kanban context build t_... [--lexical-limit 5] [--graph-limit 10] [--vector-limit 5] [--max-items 20] [--vector-config <toml>]
```

`kanban stats --json` 返回状态数量、过期 `running` 领取列表、阻塞原因聚合、
未规划的活动任务数量，以及必需步骤未完成的活动父任务数量，供本地操作人员恢复使用。
`kanban graph query` 的 SPARQL 可用 `--sparql-file <PATH|->` 从文件或标准输入读取，
并与位置参数 `<SPARQL>` 互斥。
`kanban vector query-chunks` 只接受必填的行内短文本；`query-label-atoms` 则要求在
行内文本、文本文件、行内向量 JSON 和向量 JSON 文件四种输入中选择一种。

`kanban backup` 使用 SQLite `VACUUM INTO` 创建一致备份；目标文件已存在时失败，
避免覆盖。`backup --out -` 会被明确拒绝，因为 SQLite 备份需要文件系统路径，
不能安全写入标准输出。

`kanban export --format jsonl` 导出数据库记录；目标文件已存在时失败，避免覆盖旧快照。
`export --out -` 会把 JSONL 快照写入标准输出，不输出人类可读状态文案，也不写标准错误；
该模式不能与 `--json` 组合，因为 JSONL 流和 JSON 封装不能共享标准输出。21 个稳定
判别值的输入/输出分别拥有 42 个精确模式根；每行数据闭合，必需但可为空的键不能省略，
但可显式为 `null`；导出/导入描述符与模式权威来源同源。

JSONL 不复制 `task_runs.log_path` 指向的外部日志文件，导出的运行记录会清空 `log_path`；
导出中的活动 `running` 任务会清除领取并恢复为 `ready`，对应的 `running` 运行记录会
变为 `canceled`，并追加 `task.export_sanitized` 事件解释这次可移植快照改写。需要完整
可恢复副本时使用 `kanban backup`。JSONL 导出包含通用信号账本记录类型
`signal_observation`、`signal`，以及标签本体账本记录类型
`label_ontology_observation`、`label_ontology_signal`、`label_ontology_action`、
`label_ontology_action_atom_effect` 和 `label_ontology_action_signal`；因此可移植 JSONL
与 SQLite 备份都会保留信号、本体观察/信号/操作/影响的来源记录。
JSONL `event.data.payload` 仍按不透明 JSON 保存；39 种类型的联合只属于事件 API/SSE。

`kanban import --dry-run` 会在临时 SQLite 数据库中解析导入文件并运行同一最终
`doctor` 门禁，不替换或创建所选目标数据库；脚本和 CI 可先用它验证快照。上一版导出器的
存储原生快照只作为单向兼容输入：同一记录如果同时出现自然命名的新键与对应格式的
存储原生旧键，会在兼容性规范化前以 `invalid_input` 拒绝，不能由旧值静默覆盖自然命名值。

如果 `kanban import --replace` 发现同一数据库旁存在未完成的 replacement journal，命令会
先在 held lifecycle guard 内自动恢复该 journal，再决定是否开始新的导入；恢复路径完全忽略
本次调用的 `--input`，不会打开、解析或把它当作新的 source。恢复成功是正常成功（退出码
`0`）：`--json` 输出的 `data.resumed` 为 `true`、`data.records` 为 `0`、
`data.dry_run` 为 `false`，`data.input_path` 只保留调用证据；人类输出明确写出 input
ignored。恢复失败会保留 journal/staged/previous evidence 并按运行期错误契约返回失败，不能
降级成一次 fresh import。没有 journal，或已完成 journal 在 guard 内通过完整 canonical、
previous、staged identity 校验并被 quarantine 后，才会校验 `--input` 并执行新的导入。
completed journal 若仍有 staged 文件、身份不匹配、路径不是确定性 regular file，或数据库
basename 不是 UTF-8，都会 fail closed 并返回 `invalid_input`/冲突错误；不会用回退 basename
生成可能碰撞的 journal。

`kanban import --replace` 是替换式恢复入口，必须显式传入 `--replace`；导入文件必须至少
包含一个看板，且每个看板必须包含列。该命令只能离线运行；运行前必须停止 `kanban serve`
和其他持有活动运行锁的进程；如果检测到活动运行锁会直接拒绝。导入会在同一 SQLite 事务内
执行插入与最终 `doctor` 门禁：基础关系表会校验 `task_labels`、`task_dependencies`、
`task_runs`、`task_comments`、`task_events`、`task_attachments` 的记录看板与所引用的
任务/标签/运行记录看板一致。

替换发布使用同目录的 staged SQLite 文件、previous evidence 和
`.<database-file>.replace.journal` durable journal。journal 写入前的校验失败不会改变目标；
journal 写入后任意错误、进程退出或崩溃都进入 fail-closed recovery state，不尝试用原子性
叙述掩盖部分 namespace transition，也不保证目标路径暂时存在。恢复证据（journal、staged
文件和 previous 文件）会保留；重跑相同的 `kanban import --replace` 会发现该确定性 journal
并继续恢复，只有恢复完成后才开始新的导入。已完成的 journal 会先移到同目录的 quarantine
条目，保留审计证据。所选目标数据库不存在时，成功的替换会创建它；导入或最终门禁在
journal 发布前失败时不会留下目标 placeholder，发布后则按上述恢复语义保留证据。
当前 replacement journal 的 `format_version` 为 `2`；读取、校验和恢复只接受该版本。
旧的 `v1` journal 与未知版本会在恢复开始前以 `invalid_input` fail closed，不做迁移或弱兼容
读取。

本体导入会延迟回填 `label_ontology_signals.superseded_by_signal_id` 与
`label_ontology_actions.parent_action_id`，因此不依赖 JSONL 中同表自引用记录的偶然顺序；
导入后会拒绝跨看板/孤立的通用信号上下文、通用信号替代环、跨看板本体链接、
孤立的操作—信号链接、本体替代环和操作父级环。
`kanban entity`、`kanban outbox`、`kanban derived` 是知识底座的只读维护入口。
SQLite 仍是事实源；这些命令只报告统一实体注册表、派生索引发件箱和派生存储状态，
不改变任务状态或领取。
`kanban entity list --json` 返回 `{"data": [...]}`，`kanban entity show --json` 返回
`{"data": {...}}`；两者共享闭合的公开实体项，并保留
`uri`、`kind`、`source_table`、`source_id`、`created_at`、`updated_at`，以及
必需但可为空的 `board_id`、`task_id`、`title`、`summary`、`content_hash`、
`archived_at`。调用方不能把这些字段缺失解释为 `null`。`list` 的 `--kind` 与
`--limit` 由同一 SQLite 服务查询执行；`show` 继续按精确 URI 查询并保留
`not_found` 错误封装。人类可读输出不变。

`kanban graph` 和 `kanban vector` 是辅助子进程派生层入口。源码默认 feature 图不链接
Oxigraph/LanceDB 重型依赖；Linux release cohort 为统一 maintenance runtime 显式启用
`tantivy-backend,oxigraph-backend`，但 graph/vector 命令仍按辅助进程边界解析
`KANBAN_GRAPH_HELPER` /
`KANBAN_VECTOR_HELPER`、`/usr/lib/kanban/<helper>`、CLI 同目录二进制、
`KANBAN_CARGO_TARGET_ROOT` 或 `CARGO_TARGET_DIR` 的 `release/<helper>`，最后回退到
`PATH` 中的辅助程序。辅助程序缺失或返回非法封装时，`status` 返回禁用/降级状态；
辅助程序错误封装、错误的看板/数据库/配置或载荷/领域错误会作为命令错误返回。
启用后仍只作为可重建的关系/向量存储，不参与任务状态事务。

`kanban vector status --json` 保留 `message` 兼容字段，同时返回结构化
`diagnostics`、`dirty`、`board_dirty` 字段；脏状态/错误判断应使用这些字段，不解析
`message` 文案。
`kanban vector configure` 默认写入全局配置：
`$XDG_CONFIG_HOME/kanban/config.toml`（平台默认通常为
`~/.config/kanban/config.toml`），并默认配置本机 Ollama 嵌入提供程序。
传入 `--vector-config <toml>`（别名 `--config`）时写入指定 TOML。配置命令默认调用
`/api/embed` 做短文本维度校验；校验失败时不写配置；`--skip-check` 只跳过这次
连通性/维度检查。配置格式：

```toml
board = "kanban-tool"

[vector]
provider = "ollama"
endpoint = "http://127.0.0.1:11434"
model = "qwen3-embedding:0.6b"
dimensions = 1024
```

项目级 `.kb/config.toml` 可以覆盖全局 `[vector]`；命令行
`--vector-config <toml>` 优先级最高。解析顺序是：显式 `--vector-config`、最近的项目
`.kb/config.toml`、全局配置。`kanban board use <board>` 更新项目配置文件的
`board` 字段时必须保留该文件内已有 `[vector]` 配置。配置有效且辅助程序可用时，
`kanban vector status/rebuild/sync` 使用该提供程序。`kanban context build` 通过 SQLite
补全规范任务，并在可用时合并词法、图和向量命中；图或向量未配置、不可用或失败时，
以降级标记回退到仍可用的结果。失败原因通过有界诊断信息暴露，上下文包本身仍可用。

`kanban outbox list --json` 返回 `{"data": [...]}`，每项保留完整发件箱作业字段，
包括必需但可为空的 `source_event_id` 与 `last_error`；`--status` 与 `--limit` 由同一
SQLite 服务查询执行。`kanban derived status --json` 同样返回 `{"data": [...]}`，
每个存储的 `last_rebuild_at`、`last_sync_at` 与 `last_error` 都是必需但可为空，
调用方不能把字段缺失解释为 `null`。

`kanban derived status` 中的 `last_event_id` 是存储级成功处理水位，不是当前看板的
局部水位。`dirty=true` 表示该存储仍有任意看板的待处理/运行中/失败发件箱作业，
或最近一次派生更新失败；按看板工作的 `kanban index sync`、`kanban graph sync`、
`kanban vector sync` 只清理当前看板的作业，不能因为本看板干净就强制清掉全局脏状态。

语义标签原子使用独立派生存储 `lancedb_label_atoms`，对应 LanceDB 表
`kb_label_atoms`。它不属于普通任务事件发件箱扇出：`kanban vector sync/rebuild`
只维护 `lancedb_chunks` / `kb_chunks`，不会把标签原子存储标记为完成。标签语义服务
写入 `label_semantics` / `label_atoms` 后，会单独把 `lancedb_label_atoms` 标记为脏；
提供程序或功能不可用时，该存储可报告降级，但不影响普通 `kanban label` 增删改查和
`task_labels` 绑定。

### 15.0 `kanban maintenance`

`kanban maintenance status --json` 返回 Projection v2 database identity、singleton
owner 和全部 store 状态。owner 包含实际编译的 `capabilities[]` 与
`build_identity`，但绝不返回 lease token。每个 store 另有闭合的
`runtime_availability`：`available`、`unavailable` 或 `unverified`。当前二进制缺少
backend 时使用 `unavailable` + `backend_unavailable`；活动 owner 未声明该 store
capability 时使用 `unverified` + `maintenance_owner_capability_unverified`。因此
`doctor --strict-derived` 不会把 feature-limited owner 误判为全部派生层健康。
store 的 `active_corpus`、`previous_corpus`、`building_corpus` 都是必需但可为
`null` 的字段；非空值包含 `corpus_schema`、`corpus_fingerprint`、
`embedding_model` 和 `embedding_dimensions`。LanceDB generation 必须与对应的完整
corpus binding 同时存在；从 v29 升级而来、只有 generation 而没有历史 corpus 证据的
行不会被迁移伪造绑定，并会报告 `corpus_binding_upgrade_required`，直到受控重建发布
带完整绑定的新 generation。
continuous `maintenance run` 只有在当前运行制品声明全部 projection store capability
时才会领取 singleton lease；feature-limited 制品返回 `invalid_input`，且不得留下 owner
或 lease。`run --once` 与定向 `rebuild` 仍可用于该制品实际编译的 store。

`kanban maintenance run --once --json` 和
`kanban maintenance rebuild (--all | <store>) --json` 的 `stores[]` 使用闭合
`result` 联合：成功分支为
`{"status":"succeeded","action":...,"processed":...}`；局部失败分支为
`{"status":"failed","kind":"provider|backend|delivery","message":...}`。
store 局部失败不会阻止同一 pass 尝试后续已编译 store；数据库、owner、lease/fence
或 shutdown 的全局失败仍使命令失败。脚本必须根据 `result.status` 和结构化 `kind`
判断，不解析 `message` 文案。

`maintenance rebuild` 还提供显式的预检与恢复边界：

- `--dry-run` 在数据库 lifecycle 独占边界内读取一个 checkpointed、immutable SQLite
  snapshot，并只结合当前 binary 的 runtime capability 返回 `dry_run_rebuild` 或
  `dry_run_resume` action；它不打开物理 backend、不领取 singleton/store lease、
  不 claim/ACK outbox、不创建或发布 generation。若存在非空 WAL/journal，命令
  fail closed；操作者必须先停止 writer 并通过正常 `kanban checkpoint` 收敛，而不是
  删除 sidecar；
- 若所选 store 已有 `building_generation`，普通 rebuild 与普通 dry-run 都拒绝并提示
  `--resume`，不能静默覆盖或另起 generation；
- `--resume` 只适用于单个 store，且该 store 必须已有 unfinished generation；
  `--all --resume` 在 clap parse-time 拒绝；
- continuous owner 的正常 pass 仍会自动恢复 fenced unfinished generation，
  操作者不得直接清理 SQLite building evidence。

`kanban maintenance cleanup-legacy` 只管理固定五项 allowlist：
`index/v1/tasks`、`index/v1/graph`、`index/v1/vectors`、
`index/v2/tantivy_tasks`、`index/v2/oxigraph_relations`。
DB-scoped `index/v2/databases` 不在 allowlist 内，也不能通过递归发现加入。

- 平台能力按 leaf 固定：`inventory`（即 cleanup dry-run）与 `verify` 在支持的平台上均不
  依赖 Linux `renameat2`；`inventory` 保持严格只读，`verify` 只重哈希已有 backup，
  同时仍遵守 maintenance owner exclusion。`apply` 与 `restore` 则必须使用 Linux
  fd-bound `renameat2(RENAME_NOREPLACE)`，没有非 Linux fallback；非 Linux 调用在创建或
  更新 cleanup journal、移动任一 root 之前确定性返回 `invalid_input`（退出码 2）。
- `cleanup-legacy inventory` 是严格只读 dry-run：它在同一 checkpointed、immutable
  SQLite snapshot 与 database lifecycle 独占边界内读取 database binding，再遍历五项
  root，返回 inventory 与 `inventory_digest`；非空 WAL/journal 同样 fail closed；
- `cleanup-legacy apply --backup-dir <PATH> --expected-inventory-digest <SHA256>`
  只在 maintenance owner exclusion、全部物理 writer guard、同 filesystem、
  exact digest 与安全路径检查通过后，以 crash-resumable journal 移动 root；
- 已存在 backup 时 apply 必须显式 `--resume`，不存在 backup 时 `--resume` 也会拒绝；
- `verify` 重新 hash 完整 backup；`restore` 使用同一 journal 对称恢复；
- 四个 leaf JSON 输出分别使用
  `urn:kanban-tool:schema:cli:maintenance-cleanup-legacy-{inventory|apply|verify|restore}-output:v1`
  ExactSurface contract；每个 root 是独立闭合 DTO，`action` 与 `dry_run` 分别固定为
  `inventory`/`true`、`apply`/`false`、`verify`/`false`、`restore`/`false`，其余字段包含
  `resumed`、database/backup binding、digest 与 roots。`backup_dir` 必须存在但可为 `null`，
  调用方不能把字段缺失解释为 `null`。

生产顺序、backup、previous generation、九 board 隔离、outbox SLA 与 owner restart
验收见 `docs/release/DERIVED_PROJECTION_V2_RECOVERY.md`。

上述 status/run/rebuild machine contract 都是破坏性替换后的 v2 schema root；旧 v1
artifact 已移除，不提供新旧输出双轨。

### 15.1 `kanban doctor`

检查：

- 数据库文件存在。
- 迁移完整；当前已提交的迁移版本（`schema user_version`）为 30。
- `PRAGMA integrity_check`。
- 孤立的活动运行记录。
- `running` 任务是否缺少领取。
- 过期领取数量。
- 依赖环。
- 已归档依赖边（允许“已归档父任务 → 活动子任务”作为历史；报告“活动父任务 →
  已归档子任务”）。
- 缺失的运行日志文件。
- 可疑运行日志路径。
- `ready/running` 任务带有未完成父依赖。
- `ready/running` 任务缺少可执行规格。
- `ready/running` 任务带有未来的 `scheduled_at`。
- 基础关系表看板一致性：`task_labels`、`task_dependencies`、`task_runs`、
  `task_comments`、`task_events`、`task_attachments` 的记录看板必须和所引用的
  任务/标签/运行记录看板一致。当前模式用按看板区分的复合外键保护
  `task_labels`、`task_dependencies`、`task_runs`、`task_comments` 和
  `task_attachments`；v22+ 还检查 `task_execution_plans` 的任务看板作用域，v23+ 还检查
  `task_steps` 的父任务/链接任务看板作用域。`task_events` 保留可为空的任务/运行记录引用
  与 `ON DELETE SET NULL` 语义，通过 INSERT/UPDATE 触发器校验非空引用的看板作用域。
- SQLite `PRAGMA foreign_key_check`：`doctor` 将每条违规转成硬错误问题；
  JSONL 导入最终门禁也会在提交前运行同一检查，失败时回滚整个替换事务。
- `index_outbox` 积压：`outbox_pending`、`outbox_running`、`outbox_failed`。
- 派生存储健康状态：`derived_dirty_stores`、`derived_error_stores`、
  `derived_stores[]`。每个存储包含 `dirty`、`last_error`，以及按存储目标聚合的
  待处理/运行中/失败发件箱数量。
- 基础关系一致性：人类可读输出包含 `consistency_errors` /
  `consistency_warnings` 计数；`--json` 额外返回 `consistency_issues[]`，每条问题
  包含 `severity`、`code`、`message`、`record_ids`。消息包含 `table`、`row`、
  `row_board`、`referenced` 和 `referenced_board`。非零 `consistency_errors` 会让
  `ok=false`。
- 标签本体账本健康状态：v12+ 数据库必须存在 `label_ontology_observations`、
  `label_ontology_signals`、`label_ontology_actions`、
  `label_ontology_action_atom_effects`、`label_ontology_action_signals`；`doctor`
  会报告观察/信号/操作/操作影响/操作—信号的跨看板链接、孤立链接、父操作异常、
  替代环和可检查的软引用不一致。人类可读输出包含 `ontology_ledger_errors` /
  `ontology_ledger_warnings` 计数；`--json` 额外返回 `ontology_ledger_issues[]`，
  每条问题包含 `severity`、`code`、`message`、`record_ids`。非零
  `ontology_ledger_errors` 会让 `ok=false`；警告用于可重建或可解释的软引用异常，
  不会单独让 `doctor` 变为不健康。

`dirty` / 待处理发件箱表示派生层需要同步或重建，不会改变 SQLite 中的任务事实；
失败的发件箱作业或 `last_error` 用于帮助操作人员判断是否需要运行
`kanban index sync`、`kanban graph sync/rebuild` 或 `kanban vector sync/rebuild`。
`derived_stores[].last_event_id` 表示对应存储已成功提交的全局事件水位；
当 `dirty=true` 时，它仍然只是“已成功处理到哪里”的摘要，不代表所有看板都已经干净。

---

## 16. JSON 契约索引

JSON 输出、运行期 JSON 错误、clap 参数解析阶段错误、标准错误/标准输出数据平面，
以及 JSONL / NDJSON 流式输出边界的权威契约，统一见
[1.3 JSON 输出契约](#13-json-输出契约)。

本节仅保留跳转，避免同一份 CLI 规范出现两个 JSON 契约来源。新增或修改 JSON /
JSONL / 错误码行为时，只更新 1.3 及对应命令章节，并补充测试证据。
