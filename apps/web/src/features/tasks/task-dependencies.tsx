import { TaskSelect } from './task-select';
import { useId, useState } from 'react';
import type { TaskWorkspaceState } from './use-task-workspace';
import { Icon } from '../../components/ui/icon';
import { IconButton } from '../../components/ui/button';
import { StatusIcon } from '../../components/domain/status';
import { STATUS_LABELS } from '../../domain/tasks/presentation';
import { cn } from '../../components/ui/classes';
export function TaskDependencies({ workspace }: { workspace: TaskWorkspaceState }) {
  const [expanded, setExpanded] = useState(false), id = useId();
  const model = workspace.inspectorModel;
  if (!model) return null;
  const incoming = model.parents, outgoing = model.children;
  const outstanding = incoming.filter(task => task.status !== 'done' && task.status !== 'archived');
  const visible = expanded ? incoming : incoming.slice(0, 3);
  const handlers = workspace.inspectorMutationHandlers;
  const busy = !handlers || (workspace.inspectorMutationSnapshot?.pending.size ?? 0) > 0;
  return <section className={cn('task-dependencies', outstanding.length > 0 && 'has-outstanding')} data-testid="task-dependencies" aria-labelledby={id}>
    <div className="dependencies-heading"><h3 id={id}><Icon name="link" size={15} />依赖关系</h3><span className={outstanding.length ? 'dependencies-summary pending' : 'dependencies-summary'}>{outstanding.length ? `${outstanding.length} 项前置待完成` : incoming.length ? '前置已完成' : '无前置依赖'}</span></div>
    <div className="dependency-group-label"><span>前置任务</span><small>{incoming.length}</small></div>
    {visible.map(task => <div className="dependency-row" key={task.id} data-dependency-id={task.id}><StatusIcon status={task.status} size={15} /><button className="dependency-link" onClick={()=>workspace.selectTask(task.id)}><span>{task.ref}</span><strong>{task.title}</strong></button><span className={`dependency-status status-${task.status}`}>{STATUS_LABELS[task.status]}</span><IconButton icon="close" label={`解除依赖 ${task.ref}`} disabled={busy} onClick={()=>{void handlers?.removeDependency(task.id);}} /></div>)}
    {!incoming.length && <p className="dependency-empty">没有需要先完成的任务。</p>}
    {incoming.length>3 && <button className="dependency-expand" onClick={()=>setExpanded(value=>!value)}>{expanded?'收起前置任务':`展开其余 ${incoming.length-3} 项`}</button>}
    <TaskSelect workspace={workspace} aria-label="添加依赖任务" disabled={busy} value="" placeholder="添加前置任务" exclude={[model.task.id, ...incoming.map(task=>task.id)]} onValueChange={value=>{if(value)void handlers?.addDependency(value);}} />
    <OutgoingDependencies tasks={outgoing} expanded={expanded} onExpand={()=>setExpanded(true)} onSelect={workspace.selectTask} />
  </section>;
}

function OutgoingDependencies({tasks:outgoing,expanded,onExpand,onSelect}:{tasks:NonNullable<TaskWorkspaceState['inspectorModel']>['children'];expanded:boolean;onExpand:()=>void;onSelect:(id:string)=>void}) {
 return outgoing.length>0 && <div className="outgoing-dependencies"><div className="dependency-group-label"><span>依赖此任务</span><small>{outgoing.length}</small></div>{outgoing.slice(0,expanded?outgoing.length:2).map(task=><button className="outgoing-dependency" onClick={()=>onSelect(task.id)} key={task.id}><Icon name="arrow" size={13} /><code>{task.ref}</code><span>{task.title}</span></button>)}{outgoing.length>2&&!expanded&&<button className="dependency-expand" onClick={()=>onExpand()}>显示其余 {outgoing.length-2} 项</button>}</div>;
}
