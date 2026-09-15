import { useState } from 'react';
import type { TaskListRow } from './task-list-model';
import { STATUS_LABELS, STATUS_ORDER } from '../../domain/tasks/presentation';
import { Checkbox } from '../../components/ui/input';
import { Icon } from '../../components/ui/icon';
import { EmptyState } from '../../components/ui/empty-state';
import { cn } from '../../components/ui/classes';
import { StatusIcon, PriorityBadge } from '../../components/domain/status';
import { taskOpenerKey } from '../../platform/focus/explorer-focus';

type Props={rows:readonly TaskListRow[];selected?:readonly string[];onSelect?:(ids:string[])=>void;onSelectTask:(id:string)=>void;grouped?:boolean};
function TaskRow({task,selected,onSelect,onSelectTask}:{task:TaskListRow;selected:ReadonlySet<string>;onSelect?:Props['onSelect'];onSelectTask:Props['onSelectTask']}) {
  return <tr data-testid="task-row" data-task-id={task.id} className={cn('task-row',selected.has(task.id)&&'is-selected')}>
    <td className="task-check">{onSelect?<Checkbox aria-label={'选择 '+task.ref} checked={selected.has(task.id)} onChange={event=>onSelect(event.target.checked?[...selected,task.id]:[...selected].filter(id=>id!==task.id))}/>:<StatusIcon status={task.status}/>}</td>
    <td className="task-main"><button className="task-main" data-task-opener={taskOpenerKey(task.id)} onClick={()=>onSelectTask(task.id)}>
      <span className="task-id">{task.ref}</span>{onSelect&&<StatusIcon status={task.status}/>}<span className="task-title-text">{task.title}</span>
      {task.dependencyBlocked&&<span className="task-dependency-flag" title="前置任务未完成" aria-label="前置任务未完成"><Icon name="link" size={13}/></span>}
      {task.requiredStepCount>0&&<span className="task-step-count" title="必需步骤"><Icon name="task" size={12}/>{task.completedRequiredStepCount}/{task.requiredStepCount}</span>}
    </button></td>
    <td className="task-priority"><PriorityBadge priority={task.priority}/></td>
    <td className="task-module-cell"><span className={task.assignee?'':'muted'}>{task.assignee||'未分配'}</span></td>
  </tr>;
}
export function TaskTable({rows,selected=[],onSelect,onSelectTask,grouped=true}:Props) {
  const [collapsed,setCollapsed]=useState<readonly string[]>([]);
  const selectedIds=new Set(selected),collapsedIds=new Set(collapsed);
  const toggleGroup=(id:string)=>setCollapsed(current=>{
    const next=new Set(current);
    if(!next.delete(id))next.add(id);
    return [...next];
  });
  const groups=grouped?STATUS_ORDER.flatMap(status=>{const tasks=rows.filter(task=>task.status===status);return tasks.length?[{id:status,label:STATUS_LABELS[status],tasks}]:[];}):[{id:'all' as const,label:'',tasks:rows}];
  if(!rows.length)return <EmptyState icon="task" title="没有匹配的任务" description="调整筛选条件，或创建一个新任务。"/>;
  return <table className="task-table" aria-label="任务列表" data-testid="task-list">
    <thead><tr className="task-table-head"><th className="task-check" scope="col">{onSelect&&<Checkbox aria-label="选择所有可见任务" checked={rows.every(task=>selectedIds.has(task.id))} onChange={event=>onSelect(event.target.checked?rows.map(task=>task.id):[])}/>}</th><th scope="col" className="task-main">任务</th><th scope="col" className="task-priority">优先级</th><th scope="col" className="task-module-cell">负责人</th></tr></thead>
    {groups.map(group=><tbody key={group.id}>{group.id!=='all'&&<tr><td colSpan={4}><button className="task-group-heading" aria-expanded={!collapsedIds.has(group.id)} onClick={()=>toggleGroup(group.id)}><Icon name={collapsedIds.has(group.id)?'chevron':'down'} size={13}/><StatusIcon status={group.id} size={14}/>{group.label}<span>{group.tasks.length}</span></button></td></tr>}{!collapsedIds.has(group.id)&&group.tasks.map(task=><TaskRow key={task.id} task={task} selected={selectedIds} onSelect={onSelect} onSelectTask={onSelectTask}/>)}</tbody>)}
  </table>;
}
