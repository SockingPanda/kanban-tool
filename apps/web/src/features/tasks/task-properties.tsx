import { useRef } from 'react';
import type { TaskWorkspaceState } from './use-task-workspace';
import type { useTaskInspectorState } from './use-task-detail-state';
import { inspectorActionViews } from './TaskInspector.edit-actions';
import { Icon } from '../../components/ui/icon';
import { Select } from '../../components/ui/select';
import { STATUS_LABELS, PRIORITIES } from '../../domain/tasks/presentation';

type Props = { workspace: TaskWorkspaceState; actions: ReturnType<typeof useTaskInspectorState> };
/** 只展示已接入的任务字段，可用状态流转由服务决定。 */
export function TaskProperties({ workspace, actions }: Props) {
  const trigger = useRef<HTMLButtonElement | null>(null);
  const task = actions.task, handlers = workspace.inspectorMutationHandlers;
  const available = inspectorActionViews(task, actions.claimToken, actions.copy);
  const busy = !handlers || Boolean(workspace.inspectorMutationSnapshot?.pending.size);
  const statusOptions = Object.entries(STATUS_LABELS).map(([value,label]) => {
    const action = available.find(candidate=>candidate.option.targetStatus===value&&candidate.action!=='heartbeat');
    return {value,label,disabled:value!==task.status&&!action?.enabled,description:action?.disabledReason ?? undefined};
  });
  return <div className="task-properties">
    <div><span><Icon name="circle" size={14}/>状态</span><Select aria-label="更改任务状态" value={task.status} options={statusOptions} disabled={busy} onFocus={event=>{trigger.current=event.currentTarget;}} onValueChange={status=>{
      const action=available.find(candidate=>candidate.option.targetStatus===status&&candidate.action!=='heartbeat');
      if(action?.enabled&&trigger.current)actions.openActionDialog(action,trigger.current);
    }}/></div>
    <div><span><Icon name="flag" size={14}/>优先级</span><Select aria-label="更改优先级" value={String(task.priority)} options={[...PRIORITIES]} disabled={busy} onValueChange={value=>{void handlers?.saveTask({priority:Number(value),expected_lock_version:task.lockVersion??0});}}/></div>
  </div>;
}
