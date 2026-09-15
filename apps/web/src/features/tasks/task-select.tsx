import { useState } from 'react';
import { Select, type SelectProps } from '../../components/ui/select';
import { useWorkspaceOperations } from '../../application/workspace/use-workspace-operations';
import { useAsyncRead } from '../../application/query/use-async-read';
import { defaultTaskListQuery } from '../../application/data/explorer-read-model';
import type { TaskWorkspaceState } from './use-task-workspace';

/** 关系选择独立查询整个项目，不受当前任务列表的筛选和页码限制。 */
export function TaskSelect({ workspace, exclude = [], ...props }: Omit<SelectProps, 'options'> & {
  workspace: TaskWorkspaceState;
  exclude?: readonly string[];
}) {
  const [search, setSearch] = useState('');
  const { loadTaskListPage } = useWorkspaceOperations();
  const { runtime, route, boardRevision, online } = workspace;
  const read = useAsyncRead(true, `${route.boardSlug}:${search}`, signal =>
    loadTaskListPage(runtime, route.boardSlug, { ...defaultTaskListQuery, search, limit: 100 }, { signal }),
    boardRevision, online !== false);
  const excluded = new Set(exclude);
  const options = (read.data?.tasks ?? []).flatMap(task => excluded.has(task.id) ? [] : [{ value: task.id, label: `${task.ref} · ${task.title}` }]);
  return <>
    <Select {...props} searchable onSearchChange={setSearch} options={options} />
    {read.error && <p role="alert" className="muted small">{read.error.message} <button type="button" onClick={read.retry}>重试</button></p>}
    {!read.error && read.loading && <span className="visually-hidden" role="status">正在搜索任务…</span>}
  </>;
}
