import { TaskMapChunkBoundary } from './TaskMapChunkBoundary';
import { lazy, Suspense } from 'react';
import { useTaskWorkspace, type ExplorerPageProps } from './use-task-workspace';
import { TaskPage } from './task-page';
import { TaskDetail } from './task-detail';
import { EventsView } from './EventsView';
import { TaskRunsPage } from './task-runs-page';
export type { ExplorerPageProps } from './use-task-workspace';
const TaskMapView=lazy(()=>import('./TaskMapView').then(module=>({default:module.TaskMapView})));
export function ExplorerPage(props:ExplorerPageProps) {
  const workspace=useTaskWorkspace(props);
  const {view,route,runtime}=workspace;
  return <div data-testid="explorer-page" onClickCapture={workspace.rememberTaskOpener} onKeyDownCapture={event=>{if(event.key==='Enter'||event.key===' ')workspace.rememberTaskOpener(event);}}>
    {new URLSearchParams(route.query).get('notice')==='page-removed'&&<div role="status" className="removed-page-notice">该功能已移除，已返回当前项目任务列表。</div>}
    {(view==='list'||view==='board')&&<TaskPage workspace={workspace} />}
    {view==='map'&&<TaskPage workspace={workspace}><TaskMapChunkBoundary><Suspense fallback={<p role="status">正在加载依赖图…</p>}><TaskMapView runtime={runtime} board={route.boardSlug} boardIdentity={workspace.mapIdentityRead.data} identityLoading={workspace.mapIdentityRead.loading} identityError={workspace.mapIdentityRead.error} onRetryIdentity={workspace.mapIdentityRead.retry} online={workspace.online!==false} taskId={workspace.taskId} onSelectTask={workspace.selectTask} urlState={workspace.mapUrlState} onUrlStateChange={workspace.updateMapUrlState} /></Suspense></TaskMapChunkBoundary></TaskPage>}
    {view==='runs'&&<TaskRunsPage workspace={workspace} />}
    {view==='events'&&<div className="standard-page"><EventsView onReadSettled={workspace.restoreFocus} runtime={runtime} boardSelector={route.boardSlug} taskId={workspace.taskId} kindFilter={workspace.kindFilter} online={workspace.online!==false} onKindFilterChange={workspace.updateEventKindFilter} onSelectTask={workspace.selectTask} /></div>}
    <TaskDetail workspace={workspace} />
  </div>;
}
