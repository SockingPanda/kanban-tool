import type { BoardTaskStatus } from './board';
export const STATUS_LABELS: Record<BoardTaskStatus, string> = { triage: '待分诊', todo: '待开始', scheduled: '已排期', ready: '已就绪', running: '进行中', blocked: '已阻塞', review: '待验收', done: '已完成', archived: '已归档' };
export const STATUS_ORDER: readonly BoardTaskStatus[] = ['running', 'blocked', 'review', 'ready', 'scheduled', 'todo', 'triage', 'done', 'archived'];
export const PRIORITIES = [
  { value: '0', label: '低', tone: 'gray', name: 'low' },
  { value: '1', label: '中', tone: 'blue', name: 'medium' },
  { value: '2', label: '高', tone: 'amber', name: 'high' },
  { value: '3', label: '紧急', tone: 'red', name: 'urgent' },
] as const;
