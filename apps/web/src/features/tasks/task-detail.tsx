import { useState } from 'react';
import type { TaskWorkspaceState } from './use-task-workspace';
import { useTaskInspectorState } from './use-task-detail-state';
import { TaskInspectorActionDialog, TaskInspectorEditForm } from './TaskInspectorEditActions';
import { DetailSheet } from '../../components/layout/detail-sheet';
import { Button } from '../../components/ui/button';
import { Tabs } from '../../components/ui/tabs';
import { TaskTextField } from './task-field';
import { inspectorActionViews } from './TaskInspector.edit-actions';
import { TaskDependencies } from './task-dependencies';
import { TaskProperties } from './task-properties';
import { TaskWorkContent } from './task-work-content';
import { TaskDiscussion } from './task-discussion';
import { routePath } from '../../application/navigation/router';
type ReadyWorkspace = TaskWorkspaceState & { inspectorModel: NonNullable<TaskWorkspaceState['inspectorModel']> };
const assetOperations = new Set(['addLabel', 'removeLabel', 'suggestLabels', 'uploadAttachment', 'deleteAttachment', 'downloadAttachment']);
function MutationFeedback({workspace}:{workspace:TaskWorkspaceState}) {
  const snapshot=workspace.inspectorMutationSnapshot;
  if(!snapshot)return null;
  return <>{[...snapshot.errors.entries()].flatMap(([key,error])=>assetOperations.has(error.operation)?[]:[<div className="paper-banner banner-error" key={key} role={error.kind==='stale'?'status':'alert'} data-testid="inspector-mutation-error"><span>{error.message}</span>{snapshot.retries.has(key)&&<Button size="sm" disabled={snapshot.pending.size>0} onClick={()=>{void workspace.inspectorMutationHandlers?.retry(key);}}>重试</Button>}</div>])}</>;
}
function ActionDialog({ actions }: {actions:ReturnType<typeof useTaskInspectorState>}) {
  const dialog=actions.actionDialog;
  if(!dialog)return null;
  return <TaskInspectorActionDialog dialog={dialog} locale={actions.locale} copy={actions.copy} pending={actions.mutationTransitionPending} error={actions.transitionError?.message??null} onRetry={actions.retryTransition} retryBlocksSubmit={actions.transitionRetryMatches} onDescriptionChange={description=>actions.setActionDialog(current=>current?.kind==='description'?{...current,description}:current)} onReasonChange={reason=>actions.setActionDialog(current=>current?.kind==='reason'?{...current,reason}:current)} onConfirmationChange={confirmed=>actions.setActionDialog(current=>current?.kind==='reason'?{...current,confirmed}:current)} onCancel={actions.closeActionDialog} onSubmit={actions.submitActionDialog} />;
}
function HeartbeatAction({ actions }: { actions: ReturnType<typeof useTaskInspectorState> }) {
  const heartbeat = inspectorActionViews(actions.task, actions.claimToken, actions.copy).find(view => view.action === 'heartbeat');
  if (!heartbeat) return null;
  return <Button size="sm" data-action="heartbeat" disabled={!heartbeat.enabled || actions.mutationTransitionPending} title={heartbeat.disabledReason ?? undefined} onClick={event => actions.openActionDialog(heartbeat, event.currentTarget)}>发送心跳</Button>;
}
function ReadyTaskDetail({workspace}:{workspace:ReadyWorkspace}) {
  const [tab,setTab]=useState('detail');
  const model=workspace.inspectorModel,task=model.task,handlers=workspace.inspectorMutationHandlers;
  const claimToken=workspace.taskMutations?.claimTokens?.get(task.id)??null;
  const actions=useTaskInspectorState({model,identity:workspace.inspectorIdentity,onSelectTask:workspace.selectTask,locale:workspace.locale,claimToken,mutationHandlers:handlers,mutationSnapshot:workspace.inspectorMutationSnapshot,refreshRevision:workspace.inspectorRevision,online:workspace.online!==false,onLoadRuns:workspace.loadInspectorRuns,onLoadEvents:workspace.loadInspectorEvents,onLoadNeighborhood:workspace.loadInspectorNeighborhood});
  const busy=!handlers||Boolean(workspace.inspectorMutationSnapshot?.pending.size);
  const titleSave=(title:string)=>handlers?handlers.saveTask({title:title.trim(),expected_lock_version:task.lockVersion??0}):Promise.resolve({committed:false,reconciled:false});
  return <><DetailSheet open title={task.ref} description="任务详情" className="task-detail-dialog" dismissDisabled={busy} onClose={workspace.closeInspector} closeLabel="关闭任务检查器" footer={<><Button variant="default" disabled={busy} onClick={workspace.closeInspector}>关闭</Button></>}>
    <div data-testid="task-inspector"><TaskTextField className="task-title-input" aria-label="任务标题" name="task-title" rows={2} value={task.title} save={titleSave} disabled={busy} /><TaskDependencies workspace={workspace} /><TaskProperties workspace={workspace} actions={actions} />
    <MutationFeedback workspace={workspace} />{workspace.inspectorRead.error&&<div className="paper-banner banner-error" role="alert">{workspace.inspectorRead.error.message}<Button onClick={workspace.inspectorRead.retry}>重试</Button></div>}
    <Tabs value={tab} onChange={setTab} items={[{value:'detail',label:'详情'},{value:'comments',label:'讨论',count:model.comments.length}]} />
    {tab==='detail'&&<TaskWorkContent workspace={workspace} />}{tab==='comments'&&<TaskDiscussion workspace={workspace} />}
    <details className="paper-detail-options"><summary>运行记录与任务操作</summary><HeartbeatAction actions={actions} /><Button icon="play" size="sm" onClick={()=>{void workspace.onNavigate?.(routePath({kind:'board',boardSlug:workspace.route.boardSlug,view:'runs',query:`task=${encodeURIComponent(task.id)}`},{basePath:workspace.runtime.webBasePath}));}}>查看运行记录</Button><Button size="sm" onClick={actions.beginEditor}>编辑更多属性</Button>{actions.editing&&<TaskInspectorEditForm draft={actions.editDraft} dirty={true} pending={actions.mutationSavePending} error={actions.saveError?.message??null} onRetry={actions.retrySave} retryBlocksSubmit={actions.saveRetryMatches} copy={actions.copy} onChange={actions.setEditDraft} onSave={actions.submitEditor} onCancel={actions.closeEditor} />}</details>
    </div>
  </DetailSheet><ActionDialog actions={actions} /></>;
}
export function TaskDetail({workspace}:{workspace:TaskWorkspaceState}) {
  if(!workspace.showInspector)return null;
  if(!workspace.inspectorModel)return <DetailSheet open title="任务详情" onClose={workspace.closeInspector} closeLabel="关闭任务检查器"><p role="status">{workspace.inspectorRead.error?.message??'正在加载任务详情…'}</p><Button onClick={workspace.inspectorRead.retry}>重试</Button></DetailSheet>;
  return <ReadyTaskDetail key={workspace.inspectorIdentity} workspace={{...workspace,inspectorModel:workspace.inspectorModel}} />;
}
