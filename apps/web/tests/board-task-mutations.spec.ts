import { expect, test, type Page } from '@playwright/test';
import { installExplorerFixture } from './explorer-fixture';

async function openCreate(page:Page) {
  await page.goto('/app/boards/default/list');await page.getByTestId('task-create').click();
  await page.getByTestId('task-title-input').fill('创建重试验收');
}
import { unavailable } from './rpc-fixture';

test('创建表单使用模态焦点，Escape 返回触发按钮',async({page})=>{
  await installExplorerFixture(page);await openCreate(page);
  await expect(page.getByTestId('task-mutation-dialog')).toBeVisible();
  await expect(page.getByTestId('task-title-input')).toBeFocused();
  await page.keyboard.press('Escape');await expect(page.getByTestId('task-mutation-dialog')).toHaveCount(0);
  await expect(page.getByTestId('task-create')).toBeFocused();
});

test('失败后保留创建草稿，重试沿用 task id 与幂等键',async({page})=>{
  const fixture=await installExplorerFixture(page);const bodies:Record<string,unknown>[]=[];
  fixture.rpc.handle('CreateTask', call=>{
    bodies.push(call.input);
    if(bodies.length===1)throw unavailable();
  });
  await openCreate(page);const dialog=page.getByTestId('task-mutation-dialog');
  await dialog.getByRole('button',{name:'创建任务',exact:true}).click();
  await expect(dialog.getByRole('alert')).toContainText('暂时');
  await expect(page.getByTestId('task-title-input')).toHaveValue('创建重试验收');
  await dialog.getByRole('button',{name:'重新尝试',exact:true}).click();
  await expect(dialog).toBeHidden();
  expect(bodies).toHaveLength(2);expect(bodies[1].task_id).toBe(bodies[0].task_id);expect(bodies[1].idempotency_key).toBe(bodies[0].idempotency_key);
  await expect(page).toHaveURL(/task=t_/);
});

test('首个步骤失败后仅重试步骤，已创建任务不重复提交',async({page})=>{
  const fixture=await installExplorerFixture(page);let steps=0;
  fixture.rpc.handle('CreateStep', ()=>{ steps++;if(steps===1)throw unavailable(); });
  await openCreate(page);await page.getByText('执行计划',{exact:true}).click();await page.getByTestId('first-required-step-input').fill('第一步');
  await page.getByTestId('task-mutation-dialog').getByRole('button',{name:'创建任务',exact:true}).click();
  await page.getByTestId('task-mutation-dialog').getByRole('button',{name:'重试步骤',exact:true}).click();
  await expect(page.getByTestId('task-mutation-dialog')).toBeHidden();
  expect(steps).toBe(2);expect(fixture.writeRequests.filter(path=>path==='CreateTask')).toHaveLength(1);
});

test('键盘和原生拖动都通过 claim，外部文本拖入不提交',async({page})=>{
  const fixture=await installExplorerFixture(page);let claims=0;
  fixture.rpc.handle('ClaimTask', ()=>{
    claims++;fixture.setReadyTaskStatus('running');
    return {data:{claim_token:'claim-fixture',claim_expires_at:4102444800000,task:fixture.readyTask(),run:{id:'r_claim',task_id:'t_ready',status:'running',worker_profile:'manual',worker_pid:null,claim_owner:'local',started_at:1,finished_at:null,exit_code:null,summary:null,error:null,has_log:false,metadata:{}}}};
  });
  await page.goto('/app/boards/default/board');const card=page.getByTestId('board-task').filter({hasText:'Ready task'});
  const target=page.locator('.board-column[aria-label="进行中"]');
  await target.evaluate(node=>{const data=new DataTransfer();data.setData('text/plain','t_ready');node.dispatchEvent(new DragEvent('drop',{bubbles:true,dataTransfer:data}));});
  expect(claims).toBe(0);
  await card.focus();await card.press('Space');await card.press('ArrowRight');
  await expect(card).toHaveAttribute('data-status','running');expect(claims).toBe(1);
  fixture.setReadyTaskStatus('ready');await page.reload();await expect(card).toHaveAttribute('data-status','ready');
  await card.dragTo(target);await expect(card).toHaveAttribute('data-status','running');expect(claims).toBe(2);
});

test('保存中的标题草稿等待服务，失败后可以编辑重试',async({page})=>{
  const fixture=await installExplorerFixture(page);let release!:()=>void;
  const gate=new Promise<void>(resolve=>{release=resolve;});let attempts=0;
  fixture.rpc.handle('UpdateTask', async ()=>{
    attempts++;if(attempts===1){await gate;throw unavailable();}
  });
  await page.goto('/app/boards/default/list?task=t_ready');const title=page.getByRole('textbox',{name:'任务标题',exact:true});
  await title.fill('未提交草稿');await title.press('Tab');await expect(title).toBeDisabled();
  await page.keyboard.press('Escape');await expect(page.getByTestId('task-inspector')).toBeVisible();
  release();await expect(title).toBeEnabled();await expect(title).toHaveValue('未提交草稿');
  await title.fill('重新提交草稿');await title.press('Tab');await expect.poll(()=>attempts).toBe(2);await expect(title).toBeEnabled();
  await page.reload();await expect(title).toHaveValue('重新提交草稿');
});
