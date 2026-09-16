import { useRef, useState, type FormEvent } from 'react';
import type { TaskWorkspaceState } from './use-task-workspace';
import { Button, IconButton } from '../../components/ui/button';
import { Icon } from '../../components/ui/icon';
import { Checkbox, Field, Input } from '../../components/ui/input';
import { TaskStepEditor } from './task-step-editor';
import { TaskTextField } from './task-field';
import { TaskInspectorAssetsPanel } from './TaskInspectorAssetsPanel';
export function TaskWorkContent({ workspace }: {workspace:TaskWorkspaceState}) {
  const [editingStep,setEditingStep]=useState<string|null>(null);
  const [step,setStep]=useState(''), [reason,setReason]=useState('');
  const submitting=useRef(false),handlers=workspace.inspectorMutationHandlers,model=workspace.inspectorModel;
  const busy=!handlers||Boolean(workspace.inspectorMutationSnapshot?.pending.size);
  if(!model)return null;
  const add=async(event:FormEvent)=>{event.preventDefault();if(!step.trim()||busy||submitting.current)return;const submitted=step;submitting.current=true;try{const result=await handlers.createStep({title:step.trim(),required:true});if(result.committed)setStep(current=>current===submitted?'':current);}finally{submitting.current=false;}};
  const saveDescription=(description:string)=>handlers?handlers.saveTask({description,expected_lock_version:model.task.lockVersion??0}):Promise.resolve({committed:false,reconciled:false});
  const owner=workspace.relationOwner;
  const selectedStep=owner?.relations.steps.steps.find(item=>item.id===editingStep);
  const markNotRequired=async(event:FormEvent)=>{event.preventDefault();if(!reason.trim()||busy)return;const submitted=reason;const result=await handlers.markPlanNotRequired({reason:submitted.trim()});if(result.committed)setReason(current=>current===submitted?'':current);};
  return <div className="task-detail-content">
    <Field label="任务说明"><TaskTextField aria-label="编辑任务说明" name="task-description" value={model.task.description??''} rows={6} save={saveDescription} disabled={busy} /></Field>
    <section className="detail-section" data-testid="task-inspector-steps"><h3><Icon name="task" size={16} />执行步骤 <span>{model.task.completedRequiredStepCount}/{model.task.requiredStepCount}</span></h3>
      {owner?.relations.steps.steps.map(item=><div className="step-row" key={item.id}><Checkbox aria-label={item.title} checked={item.status==='done'||item.status==='skipped'} disabled={busy||Boolean(item.linkedTask)} title={item.linkedTask?'完成状态跟随关联任务':undefined} onChange={event=>{void handlers?.mutateStep(item.id,event.target.checked?{action:"complete",input:{note:"在任务详情中完成步骤"}}:{action:"reopen",input:{reason:"在任务详情中重新打开步骤"}});}} /><button type="button" className={item.status==='done'?'step-title done-text':'step-title'} onClick={()=>setEditingStep(item.id)}>{item.title}{item.linkedTask&&<small> · {item.linkedTask.ref}</small>}{item.status==='skipped'&&<small> · 已跳过</small>}</button><IconButton icon="close" label={`删除步骤 ${item.title}`} disabled={busy} onClick={()=>{void handlers?.mutateStep(item.id,{action:"remove"});}} /></div>)}
      <form className="inline-add" onSubmit={event=>{void add(event);}}><Input name="step-title" data-testid="task-inspector-step-title" placeholder="添加执行步骤…" aria-label="新的执行步骤" value={step} onChange={event=>setStep(event.target.value)} /><Button type="submit" icon="plus" size="icon" aria-label="添加步骤" disabled={!step.trim()||busy} /></form>
      <details className="paper-detail-options"><summary>执行计划</summary><p className="muted small">{model.task.executionPlanState==='not_required'?'无需执行步骤':model.task.executionPlanState==='planned'?'已规划执行步骤':'尚未规划执行步骤'}</p><form className="inline-add" onSubmit={event=>{void markNotRequired(event);}}><Input aria-label="无需计划的原因" name="plan-not-required-reason" placeholder="说明无需执行步骤的原因…" value={reason} disabled={busy} onChange={event=>setReason(event.target.value)} /><Button type="submit" disabled={!reason.trim()||busy||model.task.executionPlanState==='not_required'}>无需计划</Button></form></details>
    </section>
    {selectedStep && <TaskStepEditor key={selectedStep.id} step={selectedStep} workspace={workspace} onClose={()=>setEditingStep(null)} />}
    {owner&&<details className="paper-detail-options"><summary>标签</summary><TaskInspectorAssetsPanel hideAttachments taskId={owner.taskId} labels={owner.data.task.labels} attachments={workspace.attachmentsRead.data??[]} suggestionResult={null} suggestionRequested={false} handlers={owner.handlers} snapshot={owner.snapshot} attachmentLoading={workspace.attachmentsRead.loading} attachmentError={workspace.attachmentsRead.error?.message??null} locale={workspace.locale} /></details>}
  </div>;
}
