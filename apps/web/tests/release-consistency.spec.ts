import { fromBinary } from "@bufbuild/protobuf"
import { WatchQueriesRequestSchema } from "../src/generated/rpc/kanban/v1/query_pb"
import { installQueryProbe } from "./release-query-probe"
import { decodeRpcRequest } from "../src/lib/rpc/codec.generated"
import { rpcRequest } from "./release-rpc"
import { mkdir, writeFile } from 'node:fs/promises';
import { expect, test, type APIRequestContext, type Page } from '@playwright/test';

const baseURL=process.env.KANBAN_RELEASE_BASE_URL!;
const runId=process.env.KANBAN_RELEASE_RUN_ID!;
const evidence=process.env.KANBAN_RELEASE_EVIDENCE_DIR!;
let alpha='',beta='',prefix='',buildId='';
const completed:string[]=[];
async function create(request:APIRequestContext,board:string,title:string) {
  const response=await rpcRequest("CreateTask", { path: { board: board }, input: {title,description:'真实一致性验收',actor:'v4-proof'} });
  expect(response.ok(),await response.text()).toBe(true);
  return (await response.json()).data;
}
async function selectProject(page:Page,slug:string) {
  await page.getByRole('button',{name:'选择项目',exact:true}).click();
  await page.getByRole('combobox',{name:'选择项目',exact:true}).selectOption(slug);
  await expect(page).toHaveURL(new RegExp(`/boards/${slug}/list`));
}

const wholeBoardReads: string[] = [];
test.beforeEach(async({page})=>{
  wholeBoardReads.length=0;
  page.on('request',request=>{
    if(request.url().endsWith('/kanban.v1.QueryService/WatchQueries')) {
      const queries=fromBinary(WatchQueriesRequestSchema, request.postDataBuffer()!.subarray(5)).queries;
      if(queries.some(query=>query.query.case==='listTasksByStatus'))wholeBoardReads.push(request.url());
    }
    if(request.url().endsWith('/kanban.v1.KanbanService/ListTasksByStatus'))wholeBoardReads.push(request.url());
  });
});
test.afterEach(async({page})=>{void page;expect(wholeBoardReads).toEqual([]);});

test.beforeAll(async({request},info)=>{
  const unique=`${info.project.name}-${Date.now()}`;
  alpha=`proof-a-${unique}`;beta=`proof-b-${unique}`;prefix=`分页 ${unique}`;
  for(const slug of [alpha,beta]) {
    const response=await rpcRequest("CreateBoard", { input: {slug,name:slug,actor:'v4-proof'} });
    expect(response.ok(),await response.text()).toBe(true);
  }
  await Promise.all(Array.from({length:12},(_,index)=>create(request,alpha,`${prefix} ${String(index).padStart(2,'0')}`)));
  await create(request,beta,`B 独立项目 ${unique}`);
  const runtime=await (await request.get(`${baseURL}/app/runtime.json`)).json();
  const manifest=await (await request.get(`${baseURL}/app/manifest.json`)).json();
  expect(runtime.webBuildId).toBe(manifest.buildId);buildId=manifest.buildId;
});
test.afterAll(async({request},info)=>{
  void request;
  await mkdir(evidence,{recursive:true});
  await writeFile(`${evidence}/consistency-${info.project.name}-${runId}.json`,JSON.stringify({baseURL,runId,buildId,alpha,beta,completed},null,2));
});

test('真实分页、搜索、看板筛选和 URL 前进后退',async({page})=>{
  await page.goto(`/app/boards/${alpha}/list?limit=10&q=${encodeURIComponent(prefix)}&sort=title`);
  const rows=page.getByTestId('task-row');
  await expect(rows).toHaveCount(10);
  const firstIds=await rows.evaluateAll(elements=>elements.map(element=>element.getAttribute('data-task-id')));
  await page.getByRole('button',{name:'下一页',exact:true}).click();
  await expect(rows).toHaveCount(2);
  await expect(page).toHaveURL(/page=2/);
  const secondIds=await rows.evaluateAll(elements=>elements.map(element=>element.getAttribute('data-task-id')));
  expect(secondIds.some(id=>firstIds.includes(id))).toBe(false);
  await page.reload();await expect(rows).toHaveCount(2);
  await page.getByRole('button',{name:'看板',exact:true}).click();
  await expect(page.getByTestId('board-task')).toHaveCount(2);
  await page.goBack();await expect(rows).toHaveCount(2);
  await page.goBack();await expect(rows).toHaveCount(10);
  await page.getByTestId('list-search').fill(`${prefix} 11`);
  await expect(rows).toHaveCount(1);
  await page.getByRole('button',{name:'看板',exact:true}).click();
  await expect(page.getByTestId('board-task')).toHaveCount(1);
  completed.push('pagination','search','filtered-board','url-history');
});

test('快速切换项目忽略旧请求的迟到结果',async({page})=>{
  let release!:()=>void,entered!:()=>void;
  const held=new Promise<void>(resolve=>{release=resolve;});
  const started=new Promise<void>(resolve=>{entered=resolve;});
  const probe=await installQueryProbe(page);
  let delayedConnection='';
  probe.onFrame(async ({connection,definition,frame})=>{
    if(!delayedConnection&&definition?.query.case==='listTasks'&&definition.query.value.board===alpha&&frame.body.case==='begin') { delayedConnection=connection; entered(); await held; }
    return 'pass' as const;
  });
  await page.goto(`/app/boards/${alpha}/list`);
  await started;
  await selectProject(page,beta);
  await expect(page.getByTestId('task-row')).toHaveCount(1);
  release();
  await expect.poll(()=>probe.actions.some(action=>action.connection===delayedConnection&&action.method==='listTasks'&&action.kind==='begin'&&action.action==='pass')).toBe(true);
  await expect(page.getByTestId('task-list')).toContainText('B 独立项目');
  await expect(page.getByTestId('task-list')).not.toContainText(prefix);
  completed.push('board-switch-late-response');
});

test('QueryService 更新与离线后的补读恢复',async({page,context,request})=>{
  await page.goto(`/app/boards/${beta}/list`);
  await expect(page.getByTestId('task-row')).toHaveCount(1);
  const external=await create(request,beta,`QueryService ${prefix}`);
  await expect(page.getByTestId('task-list')).toContainText(external.title);
  await context.setOffline(true);
  await expect(page.getByTestId('task-list')).toContainText(external.title);
  const afterDisconnect=await create(request,beta,`重连 ${prefix}`);
  await context.setOffline(false);
  await expect(page.getByTestId('task-list')).toContainText(afterDisconnect.title,{timeout:20_000});
  await page.getByTestId('nav-events').click();
  await expect(page.getByTestId('event-row').first()).toBeVisible();
  completed.push('query-subscription','offline-retains-snapshot','reconnect-catchup','activity');
});

test('设置、健康及维护确认与取消',async({page})=>{
  let backupRequests=0;
  page.on('request',request=>{if(request.url().endsWith('/kanban.v1.KanbanService/MaintenanceBackup')&&request.method()==='POST')backupRequests++;});
  await page.goto(`/app/boards/${beta}/list`);
  await page.getByTestId('nav-settings').click();
  await expect(page.getByRole('dialog',{name:'设置',exact:true})).toBeVisible();
  await page.getByRole('button',{name:'健康检查',exact:true}).click();
  await expect(page.getByTestId('health-page')).toBeVisible();
  await page.getByTestId('nav-settings').click();
  await page.getByRole('button',{name:'数据维护',exact:true}).click();
  await expect(page.getByTestId('maintenance-status')).toBeVisible();
  await page.getByTestId('maintenance-doctor-submit').click();
  await expect(page.getByTestId('maintenance-doctor-result')).toBeVisible();
  const path=`${process.env.KANBAN_RELEASE_BACKUP_DIR ?? "/tmp"}/${beta}-backup.db`;
  await page.getByTestId('maintenance-backup-path').fill(path);
  await page.getByTestId('maintenance-backup-submit').click();
  const dialog=page.getByRole('alertdialog');
  await expect(dialog).toContainText(path);
  expect(backupRequests).toBe(0);
  await dialog.getByRole('button',{name:'取消',exact:true}).click();
  expect(backupRequests).toBe(0);
  await page.getByTestId('maintenance-backup-submit').click();
  const backupResponse=page.waitForResponse(response=>response.url().endsWith('/kanban.v1.KanbanService/MaintenanceBackup')&&response.request().method()==='POST');
  await dialog.getByRole('button',{name:'继续',exact:true}).click();
  const backup=await backupResponse;
  expect(backup.ok(),await backup.text()).toBe(true);
  await expect(page.getByTestId('maintenance-backup-result')).toContainText(path);
  expect(backupRequests).toBe(1);
  completed.push('settings','health','maintenance-doctor','maintenance-confirm-cancel');
});

test('旧功能链接保留项目且不发起专属请求',async({page})=>{
  const retired:string[]=[];
  page.on('request',request=>{if(/kanban\.v1\.KanbanService\/.*(Signals|Ontology)/.test(request.url()))retired.push(request.url());});
  for(const view of ['signals','ontology']) {
    await page.goto(`/app/boards/${beta}/${view}`);
    await expect(page).toHaveURL(new RegExp(`/boards/${beta}/list`));
    await expect(page.getByText('该功能已移除，已返回当前项目任务列表。')).toBeVisible();
    await expect(page.getByTestId('task-list')).toBeVisible();
  }
  expect(retired).toEqual([]);
  completed.push('retired-links-no-requests');
});

test('真实 dispatcher 的 Run 与 stdout、stderr 日志',async({page,request},info)=>{
  const logTask=await create(request,'paper-log-proof',`日志 ${info.project.name} ${Date.now()}`);
  const planned=await rpcRequest("MarkExecutionPlanNotRequired", { path: { task_id: logTask.id }, input: {reason:'隔离日志验收',actor:'v4-proof'} });
  expect(planned.ok(),await planned.text()).toBe(true);
  const promote=await rpcRequest("PromoteTask", { path: { task_id: logTask.id }, input: {actor:'v4-proof'} });
  expect(promote.ok(),await promote.text()).toBe(true);
  await expect.poll(async()=> (await (await rpcRequest("GetTask", { path: { task_id: logTask.id } })).json()).data.status,{timeout:20_000}).toBe('done');
  await page.goto(`/app/boards/paper-log-proof/runs?task=${logTask.id}`);
  await expect(page.getByTestId('runs-ready')).toBeVisible();
  await expect(page.getByTestId('runs-log')).toContainText('paper dispatcher: real stdout');
  await expect(page.getByTestId('runs-log')).toContainText('paper dispatcher: real stderr');
  await page.screenshot({path:`${evidence}/real-log-${info.project.name}.png`,fullPage:true});
  completed.push('dispatcher','run-log-stdout-stderr');
});

test('创建失败保留草稿，真实重试只创建一个任务',async({page})=>{
  await page.goto(`/app/boards/${beta}/list`);
  const submissions:Record<string,unknown>[]=[];
  await page.route("**/kanban.v1.KanbanService/CreateTask",async route=>{
    submissions.push(decodeRpcRequest("CreateTask",route.request().postDataBuffer()!.subarray(5)).input as Record<string,unknown>);
    if(submissions.length===1)await route.abort('failed');else await route.continue();
  });
  await page.getByTestId('task-create').click();
  const dialog=page.getByTestId('task-mutation-dialog');
  const title=`保留创建草稿 ${prefix}`;
  await dialog.getByTestId('task-title-input').fill(title);
  await dialog.getByRole('button',{name:'创建任务',exact:true}).click();
  await expect(dialog.getByRole('alert')).toBeVisible();
  await expect(dialog.getByTestId('task-title-input')).toHaveValue(title);
  await dialog.getByRole('button',{name:'重新尝试',exact:true}).click();
  await expect(dialog).toBeHidden();
  expect(submissions).toHaveLength(2);
  expect(submissions[0].task_id).toBe(submissions[1].task_id);
  expect(submissions[0].idempotency_key).toBe(submissions[1].idempotency_key);
  const saved=await (await rpcRequest("GetTask", { path: { task_id: submissions[1].task_id } })).json();
  expect(saved.data.title).toBe(title);expect(saved.data.board_slug).toBe(beta);
  completed.push('create-failure-retains-draft','create-retry-idempotency');
});

test('真实看板键盘和指针拖动都经过原子 claim',async({page,request})=>{
  const titles=[`键盘 claim ${prefix}`,`指针 claim ${prefix}`];
  for(const title of titles){
    const task=await create(request,beta,title);
    for(const [method,data] of [['MarkExecutionPlanNotRequired',{reason:'隔离拖动验收',actor:'v4-proof'}],['PromoteTask',{actor:'v4-proof'}]] as const){
      const response=await rpcRequest(method,{path:{task_id:task.id},input:data});
      expect(response.ok(),await response.text()).toBe(true);
    }
  }
  await page.goto(`/app/boards/${beta}/board`);
  const target=page.locator('.board-column[aria-label="进行中"]');
  for(const [index,title] of titles.entries()){
    const card=page.getByTestId('board-task').filter({hasText:title});
    await expect(card).toHaveAttribute('data-status','ready');
    const taskId=await card.getAttribute('data-task-id');
    if(index===0){await card.focus();await card.press('Space');await card.press('ArrowRight');}
    else await card.dragTo(target);
    await expect(card).toHaveAttribute('data-status','running');
    await expect.poll(async()=> (await (await rpcRequest("GetTask", { path: { task_id: taskId } })).json()).data.status).toBe('running');
    const saved=await (await rpcRequest("GetTask", { path: { task_id: taskId } })).json();
    expect(saved.data.current_run_id).toMatch(/^r_/);
  }
  completed.push('keyboard-claim','pointer-drag-claim');
});

test('归档任务可筛选查看，归档项目不提供失效的实时入口',async({page,request})=>{
  const task=await create(request,beta,`已归档任务 ${prefix}`);
  const archivedTask=await rpcRequest("ArchiveTask", { path: { task_id: task.id }, input: {actor:'v4-proof',force:false} });
  expect(archivedTask.ok(),await archivedTask.text()).toBe(true);
  const archivedBoard=`archived-${Date.now()}`;
  const createdBoard=await rpcRequest("CreateBoard", { input: {slug:archivedBoard,name:archivedBoard,actor:'v4-proof'} });
  expect(createdBoard.ok(),await createdBoard.text()).toBe(true);
  const archived=await rpcRequest("ArchiveBoard", { path: { board: archivedBoard }, input: {actor:'v4-proof'} });
  expect(archived.ok(),await archived.text()).toBe(true);
  await page.goto(`/app/boards/${beta}/list?q=${encodeURIComponent(task.title)}`);
  await expect(page.getByText('这里还没有任务',{exact:true})).toBeVisible();
  await expect(page.getByTestId('task-row')).toHaveCount(0);
  await page.getByText('筛选与排序',{exact:true}).click();
  await page.getByRole('checkbox',{name:'包含已归档'}).check();
  await expect(page.getByTestId('task-row')).toHaveCount(1);
  await page.reload();
  await expect(page.getByTestId('task-row')).toHaveCount(1);
  await page.getByRole('button',{name:'选择项目',exact:true}).click();
  await expect(page.getByRole('option',{name:`${archivedBoard} · 已归档`,exact:true})).toBeDisabled();
  completed.push('archived-task-filter','archived-project-disabled');
});
