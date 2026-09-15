import { PageHeader } from '../../components/layout/page-header';
import { TaskRunsView } from './TaskRunsView';
import { TaskSelect } from './task-select';
import type { TaskWorkspaceState } from './use-task-workspace';

export function TaskRunsPage({ workspace }: { workspace: TaskWorkspaceState }) {
  return <div className="standard-page">
    <PageHeader eyebrow="EXECUTION HISTORY" title="运行记录" description="查看任务执行过程与日志。" />
    <div className="page-toolbar"><TaskSelect workspace={workspace} aria-label="选择运行任务" placeholder="选择任务" value={workspace.taskId ?? ''} onValueChange={workspace.selectTask} /></div>
    <TaskRunsView runtime={workspace.runtime} taskId={workspace.taskId} online={workspace.online !== false} />
  </div>;
}
