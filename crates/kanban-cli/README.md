# kanban-cli

`kanban` 是 canonical localhost host 的薄命令行 adapter。除 `serve`、本地配置/init、completion 和
hook 外，命令都通过异步 `kanban-client` 的原生 gRPC channel 请求 host；CLI 不直接打开数据库，
不实现第二套状态机。同一命令的 selector 解析和业务操作复用 channel，不创建嵌套 runtime。

## 最小路径

先运行 `kanban serve`，再用 `kanban board` 选择或查看 board，使用 `kanban task` 完成创建、查询和
显式 lifecycle 操作。需要脚本稳定输出时传 `--json`；错误使用 protocol 的 machine `error.code`，
人类消息不作为脚本判定接口。

源码树完成 Web build 后，可从仓库根目录用 `kanban serve --web-dir apps/web/dist` 启动同源 `/app/`
UI；打包安装的默认 artifact 目录是 `/usr/share/kanban-tool/web`。只需要 API host 时使用
`kanban serve --no-web`。Web 目录解析优先级为显式 `--web-dir`、`KANBAN_WEB_DIR`、默认目录。

完整领域面还包括 `label`/`labels`/`ontology`、`search`/`index`、`graph`、`vector`、`context`、
signals、attachments、runs/events 和 host-admin maintenance。`kanban label proposals list` 的
`--task-ref` 是可选的：提供时按任务列出 proposal；省略时按当前 board 查询，`--status` 可继续过滤。

完整命令、flags、alias 和退出码以 Clap 生成的 `kanban --help`、子命令 help 和 protocol DTO 为准。
README 只说明工作流和 host-admin 边界，不复制 command tree。

## 模块与迭代

先创建容器，再使用返回的 `obj_...` ID 安排任务。`--board` 接受 slug 或 ID；模块与迭代不会按
标题猜测身份。`iteration` 是 `cycle` 的别名。时间使用毫秒时间戳，创建迭代必须同时提供计划
开始和结束时间，且开始早于结束。

```sh
kanban --board default module create "交付基础能力" --request-id module-first
kanban --board default cycle create "本轮迭代" --starts-at 1790000000000 --ends-at 1790604800000 --request-id cycle-first
kanban --board default task create "实现接口" --module obj_MODULE --cycle obj_CYCLE --idempotency-key task-first
kanban --board default task list --module obj_MODULE --cycle obj_CYCLE --json
```

示例中的 `obj_MODULE`、`obj_CYCLE` 需要替换为创建结果。列表只返回精简信息，正文通过 `show`
读取；列表和成员查询支持 `--limit`、`--offset`。模块父级通过 `--parent` 设置，更新时可用
`--clear-parent` 解除。归档项需用 `--include-archived` 列出。

`task update` 的重复 `--module` 替换整个模块集合，`--clear-modules` 清空；`--cycle` 原子替换
当前迭代，`--clear-cycle` 解除归属。省略这些参数保留归属。多个模块筛选取交集，再与其他条件
共同参与总数、排序和分页。`cycle task add` 遇到其他迭代归属时拒绝，显式换属使用 `task update`。

迭代使用 `start`、`close`、`cancel` 改变生命周期；`close --carry-to obj_TARGET` 将未完成且未归档
的任务结转，不改变任务执行状态。关闭后的成员及概览标记 `frozen_snapshot`，任务列表仍筛选
当前关系。`restore` 只取消归档，保留已关闭状态。

需要重试时显式提供并保留 `--request-id`、actor 和完整参数；任务创建继续使用
`--idempotency-key`。可用任务 `--expected-lock-version`、`--expected-object-version` 和对象
`--expected-version` 做并发校验。缺省版本由服务端读取，冲突不自动覆盖。事务及快照语义见
[对象与关系](../kanban-service/docs/objects.md)。
