import { useBulkCompletion } from '../../application/tasks/use-bulk-completion';
import { useState, useEffect, useRef, useMemo, type ReactNode } from 'react';
import type { TaskWorkspaceState } from './use-task-workspace';
import type { TaskListRow } from './task-list-model';
import { useAsyncRead } from '../../application/query/use-async-read';
import { useWorkspaceOperations } from '../../application/workspace/use-workspace-operations';
import { serializeTaskListQuery, type TaskListQueryState } from '../../application/data/explorer-read-model';
import { routePath } from '../../application/navigation/router';
import { PageHeader } from '../../components/layout/page-header';
import { Button, IconButton } from '../../components/ui/button';
import { Icon } from '../../components/ui/icon';
import { Input, Checkbox } from '../../components/ui/input';
import { Select } from '../../components/ui/select';
import { Tabs } from '../../components/ui/tabs';
import { STATUS_LABELS, PRIORITIES } from '../../domain/tasks/presentation';
import { boardMessagesForLocale } from '../../domain/tasks/board';
import { TaskTable } from './task-table';
import { TaskBoard } from './task-board';
import { MutationDialog, MutationNotice } from './BoardTaskMutations';
const messages = boardMessagesForLocale('zh');
const views = [{ value: 'list', label: '列表', icon: 'list' as const }, { value: 'board', label: '看板', icon: 'grid' as const }, { value: 'map', label: '依赖图', icon: 'tree' as const }];
const sorts = [{value:'updated_at',label:'最近更新'}, {value:'-updated_at',label:'最早更新'}, {value:'priority',label:'优先级'}, {value:'title',label:'标题'}, {value:'seq',label:'任务编号'}, {value:'due_at',label:'截止时间'}];
function rowsFor(workspace: TaskWorkspaceState): TaskListRow[] { return (workspace.listRead.data?.tasks ?? []).map(task => ({ id: task.id, ref: task.ref, title: task.title, status: task.status, priority: task.priority, assignee: task.assignee, executionPlanState: task.execution_plan_state, dependencyBlocked: task.dependency_blocked, requiredStepCount: task.required_step_count, completedRequiredStepCount: task.completed_required_step_count, optionalStepCount: task.optional_step_count, updatedAt: task.updated_at })); }
function TaskStats({ workspace }: { workspace: TaskWorkspaceState }) {
  const { createMaintenanceApi } = useWorkspaceOperations();
  const { runtime, route, online } = workspace;
  const api = useMemo(() => createMaintenanceApi(undefined, runtime), [createMaintenanceApi, runtime]);
  const counts = useAsyncRead(true, route.boardSlug, signal => api.stats(route.boardSlug, signal), online !== false);
  const count = (status: string) => counts.data ? counts.data.status_counts.find(item => item.status === status)?.count ?? 0 : "—";
  return <div className="page-inline-stats"><span><b>{counts.data?.status_counts.reduce((total,item)=>total+item.count,0) ?? '—'}</b> 个任务</span><span><i className="state-dot green" /><b>{count('done')}</b> 已完成</span><span><i className="state-dot blue" /><b>{count('running')}</b> 进行中</span><span><i className="state-dot amber" /><b>{count('blocked')}</b> 已阻塞</span></div>;
}
function TaskFilters({ query, onChange }: { query: TaskListQueryState; onChange: (query: TaskListQueryState) => void }) {
  return <details className="task-extra-filters"><summary>筛选与排序</summary><div className="form-grid">
    <Select aria-label="排序" value={query.sort} options={sorts} onValueChange={sort => onChange({ ...query, sort: sort as TaskListQueryState['sort'], page: 1 })} />
    <Select aria-label="每页" value={String(query.limit)} options={[10,25,50,100].map(limit => ({value:String(limit),label:`每页 ${limit} 项`}))} onValueChange={limit => onChange({...query,limit:Number(limit),page:1})} />
    <label><Checkbox checked={query.includeArchived} onChange={event => onChange({...query,includeArchived:event.target.checked,page:1})} />包含已归档</label>
    <Select aria-label="优先级筛选" value={query.priority[0]?.toString() ?? ''} options={[{value:'',label:'所有优先级'},...PRIORITIES]} onValueChange={priority => onChange({...query,priority:priority ? [Number(priority) as 0|1|2|3] : [], page:1})} />
    <Select aria-label="执行计划筛选" value={query.plan[0] ?? ''} options={[{value:'',label:'所有执行计划'},{value:'plan_needed',label:'需要计划'},{value:'has_steps',label:'有步骤'},{value:'incomplete_required_steps',label:'必需步骤未完成'}]} onValueChange={plan => onChange({...query,plan:plan ? [plan as TaskListQueryState['plan'][number]]:[],page:1})} />
  </div></details>;
}
function TaskCollection({ workspace, selected, onSelect }: {workspace:TaskWorkspaceState; selected:readonly string[]; onSelect:(ids:string[])=>void}) {
  const read = workspace.listRead;
  if (!read.data && read.loading) return <p role="status" data-testid="task-list-loading">正在加载任务…</p>;
  if (read.error && !read.data) return <div className="paper-banner banner-error" role="alert" data-testid="task-list-error">{read.error.message}<Button onClick={read.retry}>重试</Button></div>;
  if (workspace.view === 'board' && workspace.listMutationModel) return <TaskBoard model={workspace.listMutationModel} visibleIds={rowsFor(workspace).map(task => task.id)} mutations={workspace.taskMutations} onSelectTask={workspace.selectTask} />;
  return <TaskTable rows={rowsFor(workspace)} selected={selected} onSelect={onSelect} onSelectTask={workspace.selectTask} />;
}
export function TaskPage({ workspace, children }: { workspace: TaskWorkspaceState; children?: ReactNode }) {
  const [selection, setSelection] = useState<{key:string;ids:string[]}>({key:'',ids:[]});
  const key = `${workspace.route.boardSlug}:${serializeTaskListQuery(workspace.listQuery)}`;
  const selected = selection.key === key ? selection.ids : [];
  const onSelect = (ids:string[]) => setSelection({key,ids});
  const selectedSet = new Set(selected);
  const selectedTasks = Object.values(workspace.listMutationModel?.tasksByStatus ?? {}).flat().filter(task=>selectedSet.has(task.id));
  const bulk = useBulkCompletion(workspace.route.boardSlug,selectedTasks,workspace.taskMutations,onSelect);
  const query = workspace.listQuery, controller = workspace.listMutationController;
  const createRequested = new URLSearchParams(workspace.route.query).get('create') === '1';
  const searchRequested = new URLSearchParams(workspace.route.query).get('focus') === 'search';
  const searchRef = useRef<HTMLInputElement>(null), opened = useRef(false);
  useEffect(() => { if (!createRequested) { opened.current = false; return; } if (!controller || opened.current) return; opened.current = true; controller.openCreate(); const params = new URLSearchParams(workspace.route.query); params.delete("create"); void workspace.onNavigate?.(routePath({...workspace.route, query:params.toString()}, {basePath:workspace.runtime.webBasePath}), {replace:true}); }, [controller, createRequested, workspace]);
  useEffect(() => { if (searchRequested) searchRef.current?.focus(); }, [searchRequested]);
  const total = workspace.listRead.data?.meta.total ?? 0;
  const changeView = (view:string) => { if(view==='list'||view==='board'||view==='map') void workspace.onNavigate?.(routePath({kind:'board',boardSlug:workspace.route.boardSlug,view,query:workspace.route.query},{basePath:workspace.runtime.webBasePath})); };
  return <div className="standard-page" data-testid="task-page">
    <PageHeader eyebrow="WORK ITEMS" title="任务" description="把具体工作安排到合适的位置。" actions={<Button variant="default" icon="plus" data-testid="task-create" disabled={!controller} onClick={event => controller?.openCreate(event.currentTarget)}>新建任务</Button>} />
    <TaskStats workspace={workspace} />
    <div className="page-toolbar"><Tabs variant="segment" value={workspace.view} onChange={changeView} items={views} /><div className="toolbar-filters"><Select aria-label="按迭代筛选任务" value="all" disabled title="迭代尚未接入"><option value="all">所有迭代</option></Select><Select aria-label="按模块筛选任务" value="all" disabled title="模块尚未接入"><option value="all">所有模块</option></Select><Select aria-label="按状态筛选任务" value={query.status[0] ?? 'all'} options={[{value:'all',label:'所有状态'},...Object.entries(STATUS_LABELS).map(([value,label])=>({value,label}))]} onValueChange={status=>workspace.updateListQuery({...query,status:status==='all'?[]:[status as TaskListQueryState['status'][number]],page:1})} /></div><div className="search-field"><Icon name="search" size={15} /><Input ref={searchRef} aria-label="搜索任务" data-testid="list-search" placeholder="搜索任务…" value={query.search} onChange={event=>workspace.updateListQuery({...query,search:event.target.value,page:1})} /></div></div>
    {selected.length>0 && <div className="bulk-toolbar"><strong>{selected.length} 项已选</strong><Select aria-label="批量加入迭代" disabled value=""><option value="">加入迭代</option></Select><Select aria-label="批量加入模块" disabled value=""><option value="">加入模块</option></Select><Button size="sm" icon="check" disabled={!bulk.canComplete||bulk.pending} title={bulk.canComplete?undefined:"所选任务尚不满足完成条件；需要确认的操作请在详情中执行"} onClick={()=>{void bulk.complete();}}>标为完成</Button><IconButton icon="close" label="取消选择" onClick={()=>onSelect([])} /></div>}
    <TaskConnectionNotice workspace={workspace} />
    {bulk.error&&<p className="paper-banner banner-error" role="alert">{bulk.error}</p>}
    {controller && <MutationNotice controller={controller} copy={messages} />}{children ?? <TaskCollection workspace={workspace} selected={selected} onSelect={onSelect} />}
    <div className="table-footer">显示 {workspace.listRead.data?.tasks.length ?? 0} / {total} 个任务<span>{workspace.view==='board'?'可拖动卡片修改状态；也可在详情中选择状态':'点击任务查看详情，勾选任务进行批量安排'}</span></div>
    <div className="paper-pagination"><TaskFilters query={query} onChange={workspace.updateListQuery} /><span /><Button size="sm" disabled={query.page<=1} onClick={()=>workspace.updateListQuery({...query,page:query.page-1})}>上一页</Button><span>{query.page} / {Math.max(1,Math.ceil(total/query.limit))}</span><Button size="sm" disabled={query.page*query.limit>=total} onClick={()=>workspace.updateListQuery({...query,page:query.page+1})}>下一页</Button></div>
    {controller && <MutationDialog controller={controller} copy={messages} />}
  </div>;
}

function TaskConnectionNotice({workspace}:{workspace:TaskWorkspaceState}) {
 const offline=workspace.online===false;
 const interrupted=workspace.syncStatus==='stale'||workspace.syncStatus==='circuit-open';
 const error=workspace.listRead.error;
 if(!workspace.listRead.data || (!offline&&!interrupted&&!error))return null;
 return <div className="paper-banner" role="status" data-testid="task-sync-notice"><span>{offline?'当前离线，保留最近一次任务数据。':error?.message??'同步暂时中断，正在恢复连接。'}</span><Button size="sm" onClick={workspace.listRead.retry} disabled={offline}>重试</Button></div>;
}
