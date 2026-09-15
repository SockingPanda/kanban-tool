import { useState } from 'react';
import type { TaskListRow } from './task-list-model';
import { STATUS_LABELS, STATUS_ORDER } from '../../domain/tasks/presentation';
import { Checkbox } from '../../components/ui/input';
import { Icon } from '../../components/ui/icon';
import { EmptyState } from '../../components/ui/empty-state';
import { cn } from '../../components/ui/classes';
import { StatusIcon, PriorityBadge } from '../../components/domain/status';
import { taskOpenerKey } from '../../platform/focus/explorer-focus';

type Props = { rows: readonly TaskListRow[]; selected?: readonly string[]; onSelect?: (ids: string[]) => void; onSelectTask: (id: string) => void; grouped?: boolean };
function TaskRow({ task, selected, onSelect, onSelectTask }: { task: TaskListRow; selected: ReadonlySet<string>; onSelect?: Props['onSelect']; onSelectTask: Props['onSelectTask'] }) {
  const selectedTask = selected.has(task.id);
  return <tr data-testid="task-row" data-task-id={task.id} className={cn('task-row', selectedTask && 'is-selected')}>
    <td className="task-check">{onSelect ? <Checkbox aria-label={`选择 ${task.ref}`} checked={selectedTask} onChange={event => onSelect(event.target.checked ? [...selected, task.id] : [...selected].filter(id => id !== task.id))} /> : <StatusIcon status={task.status} />}</td>
    <td className="task-main"><button className="task-main" data-task-opener={taskOpenerKey(task.id)} onClick={() => onSelectTask(task.id)}>
      <span className="task-id">{task.ref}</span>{onSelect && <StatusIcon status={task.status} />}<span className="task-title-text">{task.title}</span>
      {task.requiredStepCount > 0 && <span className="task-step-count"><Icon name="task" size={12} />{task.completedRequiredStepCount}/{task.requiredStepCount}</span>}
    </button></td>
    <td className="task-priority"><PriorityBadge priority={task.priority} /></td>
    <td className="task-module-cell" title="模块尚未接入"><span className="muted">未分组</span></td>
    <td className="task-points" title="估点尚未接入">—</td>
  </tr>;
}
export function TaskTable({ rows, selected = [], onSelect, onSelectTask, grouped = true }: Props) {
  const [collapsed, setCollapsed] = useState<readonly string[]>([]);
  const selectedIds = new Set(selected), collapsedIds = new Set(collapsed);
  const groups = grouped ? STATUS_ORDER.flatMap(status => { const tasks = rows.filter(task => task.status === status); return tasks.length ? [{ id: status, label: STATUS_LABELS[status], tasks }] : []; }) : [{ id: 'all' as const, label: '', tasks: rows }];
  if (!rows.length) return <EmptyState icon="task" title="这里还没有任务" description="创建任务，或调整筛选条件。" />;
  return <table className="task-table" aria-label="任务列表" data-testid="task-list">
    <thead><tr className="task-table-head"><th className="task-check" scope="col">{onSelect && <Checkbox aria-label="选择所有可见任务" checked={rows.every(task => selectedIds.has(task.id))} onChange={event => onSelect(event.target.checked ? rows.map(task => task.id) : [])} />}</th><th scope="col" className="task-main">任务</th><th scope="col" className="task-priority">优先级</th><th scope="col" className="task-module-cell">模块</th><th scope="col" className="task-points">估点</th></tr></thead>
    {groups.map(group => <tbody key={group.id}>{group.id !== 'all' && <tr><td colSpan={5}><button className="task-group-heading" aria-expanded={!collapsedIds.has(group.id)} onClick={() => setCollapsed(current => new Set(current).has(group.id) ? current.filter(id => id !== group.id) : [...current, group.id])}><Icon name={collapsedIds.has(group.id) ? 'chevron' : 'down'} size={13} /><StatusIcon status={group.id} size={14} />{group.label}<span>{group.tasks.length}</span></button></td></tr>}{!collapsedIds.has(group.id) && group.tasks.map(task => <TaskRow key={task.id} task={task} selected={selectedIds} onSelect={onSelect} onSelectTask={onSelectTask} />)}</tbody>)}
  </table>;
}
