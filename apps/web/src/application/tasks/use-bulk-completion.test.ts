import { expect, test, vi } from 'vitest';
import type { BoardTaskViewModel } from '../../domain/tasks/board';
import { createBoardTaskClaimTokenStore, type BoardTaskMutationSurface } from './task-mutation-state';
import { executeCompletionPlan, planCompletion } from './use-bulk-completion';

function task(id:string,status:BoardTaskViewModel['status']='review'):BoardTaskViewModel {
  return {id,seq:1,ref:id,title:id,status,description:'已完成验收',position:0,scheduledAt:null,dueAt:null,lastHeartbeatAt:null,statusReason:null,labels:[],priority:1,assignee:null,lockVersion:2,readiness:{dependencyBlocked:false,unfinishedParentCount:0,executionPlanState:'not_required',requiredStepCount:0,completedRequiredStepCount:0,optionalStepCount:0}};
}
function surface():BoardTaskMutationSurface {
  return {client:{createTask:vi.fn(),createStep:vi.fn(),updateTask:vi.fn(),transitionTask:vi.fn()},claimTokens:createBoardTaskClaimTokenStore(),onCanonicalReload:vi.fn(),onMutationCommitted:vi.fn()};
}
test('批量操作不绕过状态、步骤和确认要求',()=>{
  const mutations=surface();
  expect(planCompletion([task('review')],mutations)?.[0].command.action).toBe('complete');
  expect(planCompletion([task('todo','todo')],mutations)).toBeNull();
  const blocked=task('unfinished');
  expect(planCompletion([{...blocked,readiness:{...blocked.readiness,requiredStepCount:1}}],mutations)).toBeNull();
  expect(planCompletion([task('running','running')],mutations)).toBeNull();
});
test('部分失败只重试失败项，通知或刷新异常不会重放已提交任务',async()=>{
  const mutations=surface();
  vi.mocked(mutations.client.transitionTask).mockImplementation(async id=>{if(id==='b')throw Error('conflict');return {} as never;});
  vi.mocked(mutations.onMutationCommitted!).mockImplementation(()=>{throw Error('observer');});
  vi.mocked(mutations.onCanonicalReload!).mockRejectedValue(Error('read failed'));
  const result=await executeCompletionPlan(planCompletion([task('a'),task('b')],mutations)!,mutations);
  expect(result).toEqual({remaining:['b'],reloadFailed:true});
  expect(mutations.client.transitionTask).toHaveBeenCalledTimes(2);
  expect(mutations.onCanonicalReload).toHaveBeenCalledTimes(1);
});
