import type { BoardViewModel, BoardTaskViewModel, BoardTaskStatus } from '../../domain/tasks/board';
import { boardMessagesForLocale } from '../../domain/tasks/board';
import type { BoardTaskMutationSurface } from '../../application/tasks/task-mutation-state';
import { useBoardTaskMutationController, type BoardTaskMutationController } from '../../application/tasks/task-mutation-controller';
import { StatusIcon, PriorityBadge } from '../../components/domain/status';
import { Button, IconButton } from '../../components/ui/button';
import { Icon } from '../../components/ui/icon';
import { taskOpenerKey } from '../../platform/focus/explorer-focus';
import { MutationDialog, MutationNotice } from './BoardTaskMutations';
const columns: { id: BoardTaskStatus; label: string; accept: readonly BoardTaskStatus[] }[] = [
  { id: 'todo', label: '待开始', accept: ['triage', 'todo', 'scheduled', 'ready'] },
  { id: 'running', label: '进行中', accept: ['running', 'blocked'] },
  { id: 'review', label: '待验收', accept: ['review'] },
  { id: 'done', label: '已完成', accept: ['done', 'archived'] },
];
const dragColumns = columns.map((column, position) => ({ id: column.id, status: column.id, representedStatuses: column.accept, title: column.label, position, hidden: false }));
const messages = boardMessagesForLocale('zh');
function TaskCard({ task, controller, openTask }: { task: BoardTaskViewModel; controller: BoardTaskMutationController | null; openTask: (id: string) => void }) {
  const pending = controller?.isMutationPending;
  return <button type="button" className="task-card" data-testid="board-task" data-task-id={task.id} data-status={task.status} data-task-opener={taskOpenerKey(task.id)}
    ref={element => controller?.onTaskRef(task.id, element)} draggable={Boolean(controller) && !pending} aria-busy={pending} aria-label={`${task.ref} ${task.title}`} aria-keyshortcuts="Space Escape ArrowLeft ArrowRight Enter"
    onDragStart={event => controller?.onDragStart(task.id, event)} onDragEnd={() => controller?.onDragEnd(task.id)} onKeyDown={event => { if (event.key === "Enter" && !controller?.grabbedTaskId) return; controller?.onTaskKeyDown(task, event); }} onClick={event => { if (!event.defaultPrevented && !controller?.grabbedTaskId) openTask(task.id); }}>
    <div className="task-card-top"><span className="task-id">{task.ref}</span><PriorityBadge priority={task.priority} compact /></div><h3>{task.title}</h3>
    {task.status === 'blocked' && <span className="blocked-hint"><Icon name="warning" size={12} />需要处理阻塞</span>}
    <div className="task-card-footer"><span><span className="module-dot color-slate" />未分组</span><span>{task.readiness.requiredStepCount > 0 && <><Icon name="task" size={12} />{task.readiness.completedRequiredStepCount}/{task.readiness.requiredStepCount}</>}<span className="point-pill" title="估点尚未接入">—</span></span></div>
  </button>;
}
export function TaskBoard({ model, mutations, onSelectTask, visibleIds }: { visibleIds: readonly string[]; model: BoardViewModel; mutations?: BoardTaskMutationSurface; onSelectTask: (id: string) => void }) {
  const controller = useBoardTaskMutationController(model, mutations, dragColumns, messages);
  const visible = new Set(visibleIds);
  const tasks = Object.values(controller?.model.tasksByStatus ?? model.tasksByStatus).flat().filter(task => visible.has(task.id));
  return <section data-testid="board-view" data-state="ready">
    {controller && <MutationNotice controller={controller} copy={messages} />}
    <div className="task-board">{columns.map(column => <section key={column.id} className="board-column" onDragOver={event => controller?.onDragOver(column.id, event)} onDrop={event => controller?.onDrop(column.id, event)} aria-label={column.label}>
      <header><StatusIcon status={column.id} /><strong>{column.label}</strong><span>{tasks.filter(task => column.accept.includes(task.status)).length}</span><IconButton icon="plus" label={`在${column.label}新建任务`} disabled={!controller} onClick={event => controller?.openCreate(event.currentTarget)} /></header>
      <div className="board-column-body">{tasks.flatMap(task => column.accept.includes(task.status) ? [<TaskCard key={task.id} task={task} controller={controller} openTask={onSelectTask} />] : [])}<Button variant="ghost" icon="plus" className="board-add" disabled={!controller} onClick={event => controller?.openCreate(event.currentTarget)}>添加任务</Button></div>
    </section>)}</div>
    {controller && <><p className="visually-hidden" role="status" data-testid="task-drag-announcement">{controller.dragAnnouncement}</p><MutationDialog controller={controller} copy={messages} /></>}
  </section>;
}
