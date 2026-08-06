# kanban-cli

`kanban` 是 canonical localhost host 的薄命令行 adapter。除 `serve`、本地配置/init、completion 和
hook 外，命令都通过 `kanban-client` 请求 host；CLI 不直接打开数据库，不实现第二套状态机。

## 最小路径

先运行 `kanban serve`，再用 `kanban board` 选择或查看 board，使用 `kanban task` 完成创建、查询和
显式 lifecycle 操作。需要脚本稳定输出时传 `--json`；错误使用 protocol 的 machine `error.code`，
人类消息不作为脚本判定接口。

完整领域面还包括 `label`/`labels`/`ontology`、`search`/`index`、`graph`、`vector`、`context`、
signals、attachments、runs/events 和 host-admin maintenance。`kanban label proposals list` 的
`--task-ref` 是可选的：提供时按任务列出 proposal；省略时请求当前 board 的
`GET /api/v1/boards/:board/label-proposals`，`--status` 可继续过滤。

完整命令、flags、alias 和退出码以 Clap 生成的 `kanban --help`、子命令 help 和 protocol DTO 为准。
README 只说明工作流和 host-admin 边界，不复制 command tree。
