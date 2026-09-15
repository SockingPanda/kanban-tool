import { useLayoutEffect, useRef, useState } from 'react';
import type { BoardTaskViewModel } from '../../domain/tasks/board';
import { executeBoardTaskTransition, transitionCommandForTask, transitionForTaskTarget, type BoardTaskMutationSurface, type BoardTaskTransitionCommand } from './task-mutation-state';

type PlannedCompletion = {taskId:string; command:BoardTaskTransitionCommand};
export function planCompletion(tasks:readonly BoardTaskViewModel[], surface:BoardTaskMutationSurface|undefined): PlannedCompletion[] | null {
  if (!surface || !tasks.length) return null;
  const plan:PlannedCompletion[] = [];
  for (const task of tasks) {
    const token=surface.claimTokens?.get(task.id) ?? null;
    const option=transitionForTaskTarget(task,'done',token);
    if(!option || option.requiresConfirmation) return null;
    const command=transitionCommandForTask(task,option,{claimToken:token});
    if(!command) return null;
    plan.push({taskId:task.id,command});
  }
  return plan;
}

export async function executeCompletionPlan(plan:readonly PlannedCompletion[],surface:BoardTaskMutationSurface) {
  const results=await Promise.allSettled(plan.map(async item=>{
    await executeBoardTaskTransition(surface.client,item.taskId,item.command);
    surface.claimTokens?.delete(item.taskId);
    // 通知失败不能把已提交的任务归为失败项，否则重试会重复执行。
    try { surface.onMutationCommitted?.({kind:'transition',taskId:item.taskId}); }
    catch { /* 下方 canonical reload 负责恢复展示。 */ }
  }));
  const remaining=plan.flatMap((item,index)=>results[index].status==='rejected'?[item.taskId]:[]);
  let reloadFailed=false;
  try { await surface.onCanonicalReload?.({mutationKind:'transition'}); }
  catch { reloadFailed=true; }
  return {remaining,reloadFailed};
}

/** 每项仍走同一合法动作；部分失败只保留失败项，不重放已经提交的任务。 */
export function useBulkCompletion(identity:string,tasks:readonly BoardTaskViewModel[],surface:BoardTaskMutationSurface|undefined,onResult:(remaining:string[])=>void) {
  const [state,setState]=useState({identity:'',pending:false,error:''});
  const scope=useRef({identity,surface,onResult,mounted:true});
  const inFlight=useRef(new Set<string>());
  useLayoutEffect(()=>{scope.current={identity,surface,onResult,mounted:true};return()=>{scope.current.mounted=false;};});
  const plan=planCompletion(tasks,surface);
  const complete=async()=>{
    if(!plan || !surface || inFlight.current.has(identity))return;
    inFlight.current.add(identity);
    setState({identity,pending:true,error:''});
    const current=()=>scope.current.mounted && scope.current.identity===identity && scope.current.surface===surface;
    try {
      const {remaining,reloadFailed}=await executeCompletionPlan(plan,surface);
      if(!current())return;
      scope.current.onResult(remaining);
      if(reloadFailed)setState({identity,pending:true,error:'状态已提交，刷新失败；请重新加载任务列表。'});
      if(remaining.length)setState({identity,pending:true,error:`${remaining.length} 项未完成，请查看任务详情后重试。`});
    } finally {
      inFlight.current.delete(identity);
      if(current())setState(value=>({...value,pending:false}));
    }
  };
  return {canComplete:plan!==null,pending:state.identity===identity && state.pending,error:state.identity===identity?state.error:'',complete};
}
