import { useId } from 'react';
import type { BoardMessages } from '../../domain/tasks/board';
import { PRIORITIES, STATUS_LABELS } from '../../domain/tasks/presentation';
import type { BoardTaskMutationController, MutationDialog } from '../../application/tasks/task-mutation-controller';
import { Button } from '../../components/ui/button';
import { Dialog } from '../../components/ui/dialog';
import { Field, Input, Textarea } from '../../components/ui/input';
import { Select } from '../../components/ui/select';

type Props = { controller: BoardTaskMutationController; copy: BoardMessages; dialog: Extract<MutationDialog, { kind: 'create' }> };
const createStatuses = ['triage', 'todo', 'scheduled'] as const;
export function TaskForm({ controller, copy, dialog }: Props) {
  const id=useId(), pending=controller.isPending('create'), status=dialog.status??'todo';
  return <Dialog open title="新建任务" testId="task-mutation-dialog" dismissDisabled={pending} onClose={controller.closeDialog} footer={<>
    <Button disabled={pending} onClick={controller.closeDialog}>取消</Button>
    <Button type="submit" form={id} variant="default" disabled={pending}>{pending?'正在创建…':dialog.taskCreated?'重试步骤':'创建任务'}</Button>
  </>}>
    {controller.notice&&<div role="alert" className="paper-banner banner-error"><span>{controller.notice.message}</span>{controller.retryIntent&&<Button onClick={controller.retryMutation} disabled={pending} size="sm">{dialog.taskCreated?copy.retryCreateStep:copy.retryMutation}</Button>}</div>}
    <form id={id} onSubmit={event=>{event.preventDefault();controller.submitDialog();}} className="form-stack">
      <Field label="任务标题" htmlFor={id+'-title'}><Input id={id+'-title'} name="task-title" autoFocus required placeholder="需要完成什么？" value={dialog.title} disabled={pending||dialog.taskCreated} onChange={event=>controller.setDialogTitle(event.target.value)} data-testid="task-title-input"/></Field>
      <Field label="说明" htmlFor={id+'-description'}><Textarea id={id+'-description'} name="task-description" rows={4} placeholder="目标、边界与验收要求…" value={dialog.description} disabled={pending||dialog.taskCreated} onChange={event=>controller.setDialogDescription(event.target.value)} data-testid="task-description-input"/></Field>
      <div className="form-grid">
        <Field label="状态" htmlFor={id+'-status'}><Select id={id+'-status'} aria-label="新任务状态" value={status} disabled={pending||dialog.taskCreated} onValueChange={value=>controller.setDialogCreateOptions({status:value as 'triage'|'todo'|'scheduled'})} options={createStatuses.map(value=>({value,label:STATUS_LABELS[value]}))}/></Field>
        <Field label="优先级" htmlFor={id+'-priority'}><Select id={id+'-priority'} aria-label="新任务优先级" value={String(dialog.priority??1)} disabled={pending||dialog.taskCreated} onValueChange={value=>controller.setDialogCreateOptions({priority:Number(value)})} options={[...PRIORITIES]}/></Field>
      </div>
      {status==='scheduled'&&<Field label="排期时间" htmlFor={id+'-schedule'}><Input id={id+'-schedule'} aria-label="排期时间" type="datetime-local" required value={dialog.scheduledAt??''} disabled={pending||dialog.taskCreated} onChange={event=>controller.setDialogCreateOptions({scheduledAt:event.target.value})}/></Field>}
      <details className="paper-detail-options"><summary>执行计划</summary><Field label="第一个必需步骤" htmlFor={id+'-step'}><Input id={id+'-step'} name="first-required-step" placeholder="添加执行步骤…" value={dialog.firstStepTitle} disabled={pending} onChange={event=>controller.setDialogFirstStepTitle(event.target.value)} data-testid="first-required-step-input"/></Field></details>
    </form>
  </Dialog>;
}
