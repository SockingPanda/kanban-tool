import type { BoardTaskStatus } from '../../domain/tasks/board';
import { PRIORITIES, STATUS_LABELS } from '../../domain/tasks/presentation';
import { Icon, type IconName } from '../ui/icon';
const statusIcons: Record<BoardTaskStatus, IconName> = { triage: 'box', todo: 'circle', scheduled: 'calendar', ready: 'check', running: 'active', review: 'eye', done: 'checkCircle', blocked: 'warning', archived: 'archive' };
export function StatusIcon({ status, size = 16 }: { status: BoardTaskStatus; size?: number }) {
  return <Icon name={statusIcons[status]} size={size} className={`status-icon status-${status}`} />;
}
export function StatusBadge({ status }: { status: BoardTaskStatus }) { return <span className={`status-badge status-${status}`}><StatusIcon status={status} size={14} />{STATUS_LABELS[status]}</span>; }
export function PriorityBadge({ priority, compact = false }: { priority: number; compact?: boolean }) {
  const level = PRIORITIES[priority] ?? PRIORITIES[0];
  return <span className={`priority priority-${level.name}`} title={`${level.label}优先级 · P${priority}`}><span className="priority-bars" aria-hidden="true"><i /><i /><i /></span>{!compact && level.label}</span>;
}
