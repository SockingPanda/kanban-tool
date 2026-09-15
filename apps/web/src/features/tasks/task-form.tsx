import { useId } from 'react';
import type { BoardMessages } from '../../domain/tasks/board';
import { PRIORITIES, STATUS_LABELS } from '../../domain/tasks/presentation';
import type { BoardTaskMutationController, MutationDialog } from '../../application/tasks/task-mutation-controller';
import { Button } from '../../components/ui/button';
import { Dialog } from '../../components/ui/dialog';
import { Field, Input, Textarea } from '../../components/ui/input';
import { Select } from '../../components/ui/select';

type Props = { controller: BoardTaskMutationController; copy: BoardMessages; dialog: Extract<MutationDialog, { kind: 'create' }> };
export function TaskForm({ controller, copy, dialog }: Props) {
  const id = useId();
  const pending = controller.isPending('create');
  const status = dialog.status ?? 'todo';
  return <Dialog open title="新建任务" description="任务、模块、迭代与地图使用同一份数据。" testId="task-mutation-dialog" dismissDisabled={pending} onClose={controller.closeDialog} footer={<>
    <Button disabled={pending} onClick={controller.closeDialog}>取消</Button>
    <Button type="submit" form={id} variant="default" disabled={pending}>{pending ? '正在创建…' : dialog.taskCreated ? '重试步骤' : '创建任务'}</Button>
  </>}>
    {controller.notice && <div role="alert" className="paper-banner banner-error"><span>{controller.notice.message}</span>{controller.retryIntent && <Button onClick={controller.retryMutation} disabled={pending} size="sm">{dialog.taskCreated ? copy.retryCreateStep : copy.retryMutation}</Button>}</div>}
    <form id={id} onSubmit={event => { event.preventDefault(); controller.submitDialog(); }} className="form-stack">
      <Field label="任务标题" htmlFor={`${id}-title`}><Input id={`${id}-title`} name="task-title" autoFocus required placeholder="需要完成什么？" value={dialog.title} disabled={pending || dialog.taskCreated} onChange={event => controller.setDialogTitle(event.target.value)} data-testid="task-title-input" /></Field>
      <Field label="说明" htmlFor={`${id}-description`}><Textarea id={`${id}-description`} name="task-description" rows={3} placeholder="目标、边界与验收要求…" value={dialog.description} disabled={pending || dialog.taskCreated} onChange={event => controller.setDialogDescription(event.target.value)} data-testid="task-description-input" /></Field>
      <div className="form-grid">
        <Field label="状态"><Select aria-label="新任务状态" value={status} disabled={pending || dialog.taskCreated} onValueChange={value => controller.setDialogCreateOptions({ status: value as 'triage' | 'todo' | 'scheduled' })} options={Object.entries(STATUS_LABELS).map(([value, label]) => ({ value, label, disabled: !['triage', 'todo', 'scheduled'].includes(value) }))} /></Field>
        <Field label="优先级"><Select aria-label="新任务优先级" value={String(dialog.priority ?? 1)} disabled={pending || dialog.taskCreated} onValueChange={value => controller.setDialogCreateOptions({ priority: Number(value) })} options={[...PRIORITIES]} /></Field>
      </div>
      {status === 'scheduled' && <Field label="排期时间"><Input aria-label="排期时间" type="datetime-local" required value={dialog.scheduledAt ?? ''} disabled={pending || dialog.taskCreated} onChange={event => controller.setDialogCreateOptions({ scheduledAt: event.target.value })} /></Field>}
      <Field label="所属迭代"><Select aria-label="所属迭代" disabled value="" title="迭代尚未接入"><option value="">暂不安排</option></Select></Field>
      <Field label="主要能力节点"><Select aria-label="主要能力节点" disabled value="" title="项目能力地图尚未接入"><option value="">尚未关联地图</option></Select></Field>
      <Field label="所属模块" hint="可同时归属多个模块；项目总计按任务 ID 去重。"><div className="checkbox-chips"><span className="muted small">尚未接入</span></div></Field>
      <details className="paper-detail-options"><summary>执行计划</summary><Field label="第一个必需步骤"><Input name="first-required-step" placeholder="添加执行步骤…" value={dialog.firstStepTitle} disabled={pending} onChange={event => controller.setDialogFirstStepTitle(event.target.value)} data-testid="first-required-step-input" /></Field></details>
    </form>
  </Dialog>;
}
