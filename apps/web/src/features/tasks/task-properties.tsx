import { useRef } from 'react';
import type { TaskWorkspaceState } from './use-task-workspace';
import type { useTaskInspectorState } from './use-task-detail-state';
import { inspectorActionViews } from './TaskInspector.edit-actions';
import { Icon } from '../../components/ui/icon';
import { Select } from '../../components/ui/select';
import { Field, Input } from '../../components/ui/input';
import { STATUS_LABELS, PRIORITIES } from '../../domain/tasks/presentation';
type Props = { workspace: TaskWorkspaceState; actions: ReturnType<typeof useTaskInspectorState> };
export function TaskProperties({ workspace, actions }: Props) {
  const trigger = useRef<HTMLButtonElement | null>(null);
  const task = actions.task, handlers = workspace.inspectorMutationHandlers;
  const available = inspectorActionViews(task, actions.claimToken, actions.copy);
  const busy = !handlers || Boolean(workspace.inspectorMutationSnapshot?.pending.size);
  const statusOptions = Object.entries(STATUS_LABELS).map(([value,label]) => {
    const action = available.find(candidate=>candidate.option.targetStatus===value&&candidate.action!=='heartbeat');
    return {value,label,disabled:value!==task.status&&!action?.enabled,description:action?.disabledReason ?? undefined};
  });
  return <><div className="task-properties">
    <div><span><Icon name="circle" size={14} />状态</span><Select aria-label="更改任务状态" value={task.status} options={statusOptions} disabled={busy} onFocus={event=>{trigger.current=event.currentTarget;}} onValueChange={status=>{const action=available.find(candidate=>candidate.option.targetStatus===status&&candidate.action!=='heartbeat');if(action?.enabled&&trigger.current)actions.openActionDialog(action,trigger.current);}} /></div>
    <div><span><Icon name="flag" size={14} />优先级</span><Select aria-label="更改优先级" value={String(task.priority)} options={[...PRIORITIES]} disabled={busy} onValueChange={value=>{void handlers?.saveTask({priority:Number(value),expected_lock_version:task.lockVersion??0});}} /></div>
    <div><span><Icon name="cycle" size={14} />迭代</span><Select aria-label="更改所属迭代" value="" disabled title="迭代尚未接入"><option value="">尚未安排</option></Select></div>
    <div><span><Icon name="map" size={14} />主要能力</span><Select aria-label="更改主要能力" value="" disabled title="项目能力地图尚未接入"><option value="">尚未关联</option></Select></div>
    <div><span><Icon name="grid" size={14} />估点</span><Input aria-label="更改估点" type="number" min="0" max="100" value="" disabled placeholder="—" title="估点尚未接入" /></div>
  </div><Field label="模块" hint="相关工作的交付集合，可跨迭代完成。与主要能力独立关联。"><div className="checkbox-chips compact"><span className="muted small">尚未接入</span></div></Field></>;
}
