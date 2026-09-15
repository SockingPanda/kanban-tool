import { useState } from 'react';
import type { TaskWorkspaceState } from './use-task-workspace';
import { Dialog } from '../../components/ui/dialog';
import { Button } from '../../components/ui/button';
import { Input, Textarea, Field, Checkbox } from '../../components/ui/input';
import { TaskSelect } from './task-select';

type Step = NonNullable<TaskWorkspaceState['relationOwner']>['relations']['steps']['steps'][number];
export function TaskStepEditor({ workspace, step, onClose }: { workspace: TaskWorkspaceState; step: Step; onClose: () => void }) {
  const [title, setTitle] = useState(step.title);
  const [body, setBody] = useState(step.body ?? '');
  const [required, setRequired] = useState(step.required);
  const [linked, setLinked] = useState(step.linkedTask?.id ?? '');
  const [reason, setReason] = useState('');
  const handlers = workspace.inspectorMutationHandlers;
  const busy = !handlers || Boolean(workspace.inspectorMutationSnapshot?.pending.size);
  const save = async () => {
    if (busy || !title.trim()) return;
    const result = await handlers.mutateStep(step.id, { action: 'update', input: {
      title: title.trim(), body, required, unlink_task: !linked && Boolean(step.linkedTask),
      ...(linked ? { linked_task_ref: linked } : step.linkedTask ? { unlink_task: true } : {}),
    } });
    if (result.committed) onClose();
  };
  return <Dialog open title="执行步骤" onClose={onClose} dismissDisabled={busy} footer={<><Button disabled={busy} onClick={onClose}>取消</Button><Button variant="default" disabled={busy || !title.trim()} onClick={() => { void save(); }}>保存</Button></>}>
    <div className="form-stack">
      {[...(workspace.inspectorMutationSnapshot?.errors.values() ?? [])].map(error=><p className="paper-banner banner-error" role="alert" key={error.message}>{error.message}</p>)}
      <Field label="步骤标题"><Input aria-label="步骤标题" value={title} disabled={busy} onChange={event => setTitle(event.target.value)} /></Field>
      <Field label="步骤说明"><Textarea aria-label="步骤说明" rows={4} value={body} disabled={busy} onChange={event => setBody(event.target.value)} /></Field>
      <label className="checkbox-row"><Checkbox checked={required} disabled={busy} onChange={event => setRequired(event.target.checked)} />必需步骤</label>
      <Field label="关联任务"><TaskSelect workspace={workspace} aria-label="关联步骤任务" placeholder={step.linkedTask ? `${step.linkedTask.ref} · ${step.linkedTask.title}` : '选择任务'} value={linked} exclude={[workspace.taskId ?? '']} disabled={busy} onValueChange={setLinked} />{linked && <Button size="sm" disabled={busy} onClick={() => setLinked('')}>解除关联</Button>}</Field>
      {!step.linkedTask && step.status === 'todo' && <Field label="跳过原因"><Input aria-label="跳过原因" value={reason} disabled={busy} onChange={event => setReason(event.target.value)} /><Button size="sm" disabled={busy || !reason.trim()} onClick={() => { void handlers?.mutateStep(step.id, { action: 'skip', input: { reason: reason.trim() } }).then(result => { if (result.committed) onClose(); }); }}>跳过步骤</Button></Field>}
    </div>
  </Dialog>;
}
