# 迁移与导入

本页说明 `kanban-service` 负责的升级、导入和恢复边界。当前 schema 的精确 lineage、checksum 和
column shape 由 migration implementation 持有；任务中的迁移进度不写入长期指南。

## 升级

启动 host 时先验证数据库 family、lineage、约束和外键，再在 service-owned transaction 中应用需要的
upgrade。升级前创建并验证 sibling backup；校验失败或 migration 出错时保持 fail-closed，并保留原始
事实和可再次启动的 backup。重复启动应保持幂等。

## Portable 导入

portable JSONL import/export 只交换 canonical facts。`import_journal` 记录 source fingerprint、阶段
和错误，使 staging、校验、发布和重启恢复可审计；替换模式必须先完成 verified backup，事务失败回滚。
导出的 label/ontology/proposal、signal、relation 和 attachment metadata 与任务事实一起迁移；FTS、
vector、graph、context 等派生结果在提交后由 host 重建，不缩减产品功能面。

## Legacy SQLite 导入

legacy SQLite 只作为只读输入，通过 `legacy-sqlite-import` feature 编译，并由 host-admin path 调用。
service 先做 schema、引用、计数和 board isolation 预检，再把附件写入同文件系统 staging、校验 hash，
按依赖顺序插入 canonical facts 并记录 journal。该 feature 未启用时返回 `feature_not_available`。

## 恢复边界

恢复操作只作用于 host-owned 数据、journal 和 verified backup。历史 release runbook 与已完成迁移记录
不属于 active documentation tree；历史由 Git/tag、release asset 或 task record 持有。
