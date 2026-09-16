import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'
import { I64_MIN, I64_MAX, U64_MAX } from '../../domain/integer'
import { stringifyJson } from '../../lib/lossless-json'
import { formatAttachmentSize } from '../../application/tasks/attachment-intents'
import { buildInspectorSaveTaskInput, inspectorEditDraft, inspectorRetryIntentMatches } from './TaskInspector.edit-actions'
import type { TaskInspectorViewModel } from './inspector-model'
import { commentPageState, formatCommentDateTime } from './TaskInspectorRelationsPanel.logic'
import { TaskTable } from './task-table'

const task: TaskInspectorViewModel['task'] = {
  id: 't_large', ref: 'large#9223372036854775807', title: '原题', status: 'todo',
  lockVersion: I64_MAX, scheduledAt: I64_MIN, dueAt: I64_MAX, priority: 2,
  description: '说明', statusReason: null, assignee: null, executionPlanState: 'not_required',
  dependencyBlocked: false, unfinishedParentCount: 0, requiredStepCount: I64_MAX,
  completedRequiredStepCount: I64_MAX - 1n, optionalStepCount: 0, metadata: { n: U64_MAX },
  claimOwner: null, claimExpiresAt: null, lastHeartbeatAt: I64_MAX, currentRunId: null,
  retryCount: 0, maxRetries: null, createdAt: I64_MIN, updatedAt: I64_MAX,
}

describe('页面整数展示、草稿和回传', () => {
  test('保存标题保留超 Date 范围的原始时间、CAS 和正常时间戳的毫秒', () => {
    const draft = inspectorEditDraft(task)
    expect(draft.scheduledAt).toBe(String(I64_MIN))
    expect(draft.dueAt).toBe(String(I64_MAX))
    const saved = buildInspectorSaveTaskInput(task, { ...draft, title: '新题' })
    expect(saved).toMatchObject({ title: '新题', expected_lock_version: I64_MAX, scheduled_at: I64_MIN, due_at: I64_MAX })
    const regular = { ...task, scheduledAt: 1750000000123, dueAt: 1750000060789 }
    expect(buildInspectorSaveTaskInput(regular, { ...inspectorEditDraft(regular), title: '新题' })).toMatchObject({ scheduled_at: regular.scheduledAt, due_at: regular.dueAt })
    const changed = buildInspectorSaveTaskInput(task, { ...draft, dueAt: String(I64_MAX - 1n) })
    expect(changed?.due_at).toBe(I64_MAX - 1n)
    expect(buildInspectorSaveTaskInput(task, { ...draft, dueAt: '' })?.due_at).toBeNull()
    expect(buildInspectorSaveTaskInput(task, { ...draft, dueAt: String(I64_MAX + 1n) })).toBeNull()
    expect(buildInspectorSaveTaskInput(task, { ...draft, dueAt: '错误时间' })).toBeNull()
    expect(inspectorEditDraft({ ...task, dueAt: 8_640_000_000_000_000 }).dueAt).toBe('8640000000000000')
  })

  test('重试草稿比较支持 bigint，并区分相邻版本、动态整数和字符串', () => {
    const input = buildInspectorSaveTaskInput(task, inspectorEditDraft(task))
    if (input === null) throw new Error('有效 fixture 必须可保存')
    const intent = { operation: 'saveTask' as const, taskId: task.id, input }
    expect(inspectorRetryIntentMatches(intent, 'saveTask', input)).toBe(true)
    expect(inspectorRetryIntentMatches(intent, 'saveTask', { ...input, expected_lock_version: I64_MAX - 1n })).toBe(false)
    const metadata = { ...input, metadata: { n: U64_MAX } }
    expect(inspectorRetryIntentMatches({ operation: 'saveTask', taskId: task.id, input: metadata }, 'saveTask', { ...input, metadata: { n: String(U64_MAX) } })).toBe(false)
  })

  test('评论相邻大时间戳按整数排序与分页，超范围时间与附件大小显示完整十进制', () => {
    const comments = [{ id: 'b', createdAt: 9007199254740993n }, { id: 'a', createdAt: 9007199254740992n }, { id: 'c', createdAt: 1 }]
    expect(commentPageState(comments, 1, 1, 'oldest').comments.map(item => item.id)).toEqual(['a'])
    expect(commentPageState(comments, 0, 1, 'newest').comments.map(item => item.id)).toEqual(['b'])
    expect(formatCommentDateTime(I64_MIN, 'zh').label).toBe(String(I64_MIN))
    expect(formatAttachmentSize(I64_MAX)).toBe(`${I64_MAX} B`)
    const text = renderToStaticMarkup(<pre>{stringifyJson({ integer: U64_MAX, text: String(U64_MAX) }, 2)}</pre>)
    expect(text).toContain('18446744073709551615')
    expect(text).toContain('&quot;text&quot;: &quot;18446744073709551615&quot;')
  })

  test('真实列表组件显示完整步骤计数', () => {
    const markup = renderToStaticMarkup(<TaskTable rows={[{ ...task, updatedAt: I64_MAX }]} onSelectTask={() => {}} />)
    expect(markup).toContain('9223372036854775806/9223372036854775807')
    expect(markup).toContain(task.ref)
  })
})
