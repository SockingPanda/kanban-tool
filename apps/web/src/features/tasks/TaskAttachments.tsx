import type { TaskWorkspaceState } from './use-task-workspace';
import { useAttachmentTransfer } from '../../application/tasks/use-attachment-transfer';
import { AttachmentManager } from '../attachments';

export function TaskAttachments({ workspace, onBusyChange }: { workspace: TaskWorkspaceState; onBusyChange?: (busy: boolean) => void }) {
  const client = useAttachmentTransfer(workspace.runtime, workspace.route.boardSlug);
  const owner = workspace.relationOwner;
  if (!owner) return null;
  return <AttachmentManager key={workspace.inspectorIdentity} taskId={owner.taskId} client={client}
    attachments={workspace.attachmentsRead.data ?? []} loading={workspace.attachmentsRead.loading}
    error={workspace.attachmentsRead.error?.message ?? null} locale={workspace.locale}
    disabled={workspace.online === false || owner.snapshot.pending.size > 0}
    reconcile={async () => { await workspace.attachmentsRead.reload(); }}
    remove={attachmentId => owner.handlers.deleteAttachment({ attachmentId })}
    onBusyChange={onBusyChange} />;
}
