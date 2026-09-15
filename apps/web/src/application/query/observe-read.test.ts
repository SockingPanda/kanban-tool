import { describe, expect, test, vi } from 'vitest'
import { observeRead, readObservedDependency, type ReadDependency } from './observe-read'

function dependency<T>(key: string, initial: T) {
  let value = initial
  let version = 0
  const listeners = new Set<() => void>()
  const dependency: ReadDependency<T> = {
    key, version: () => version, read: async () => ({ value, version }), retry: vi.fn(),
    subscribe: listener => { listeners.add(listener); return () => { listeners.delete(listener) } },
  }
  return { dependency, listeners, commit: (next: T) => { value = next; version += 1; for (const listener of listeners) listener() } }
}

describe('按 query 依赖映射复合读模型', () => {
  test('前置查询暂时失败保留后续依赖，恢复沿用订阅，卸载仍释放全部', async () => {
    const board = dependency('board', 'A')
    const comments = dependency('comments', '已提交评论')
    let unavailable = false
    const guardedBoard: ReadDependency<string> = { ...board.dependency, read: async () => {
      if (unavailable) throw new Error('连接中断')
      return board.dependency.read()
    } }
    const controller = new AbortController()
    const next = vi.fn(), failed = vi.fn()
    observeRead(async signal => {
      await readObservedDependency(signal, guardedBoard)
      return readObservedDependency(signal, comments.dependency)
    }, controller.signal, next, failed)
    await vi.waitFor(() => expect(next).toHaveBeenLastCalledWith('已提交评论'))
    unavailable = true
    board.commit('A')
    await vi.waitFor(() => expect(failed).toHaveBeenCalledOnce())
    expect(comments.listeners.size).toBe(1)
    comments.commit('恢复后可见的完整评论')
    unavailable = false
    board.commit('A')
    await vi.waitFor(() => expect(next).toHaveBeenLastCalledWith('恢复后可见的完整评论'))
    expect(comments.listeners.size).toBe(1)
    controller.abort()
    expect(board.listeners.size + comments.listeners.size).toBe(0)
  })

  test('保留活跃依赖、仅相关变更重算，分支切换释放旧 log，卸载完全释放', async () => {
    const selected = dependency('runs', 'log1')
    const one = dependency('log1', '第一份日志')
    const two = dependency('log2', '第二份日志')
    const controller = new AbortController()
    const next = vi.fn()
    observeRead(async signal => {
      const id = await readObservedDependency(signal, selected.dependency)
      return readObservedDependency(signal, id === 'log1' ? one.dependency : two.dependency)
    }, controller.signal, next, error => { throw error })
    await vi.waitFor(() => expect(next).toHaveBeenLastCalledWith('第一份日志'))
    two.commit('未订阅追加')
    expect(next).toHaveBeenCalledTimes(1)
    selected.commit('log2')
    await vi.waitFor(() => expect(next).toHaveBeenLastCalledWith('未订阅追加'))
    expect(one.listeners.size).toBe(0)
    expect(two.listeners.size).toBe(1)
    one.commit('旧日志迟到')
    expect(next).toHaveBeenCalledTimes(2)
    two.commit('第二份日志追加')
    await vi.waitFor(() => expect(next).toHaveBeenLastCalledWith('第二份日志追加'))
    controller.abort()
    expect(selected.listeners.size + one.listeners.size + two.listeners.size).toBe(0)
  })

  test('异步映射途中依赖变化时丢弃混合结果', async () => {
    const source = dependency('task', { title: '原始', lockVersion: 1 })
    let release: (() => void) | undefined
    let pause = true
    const wait = new Promise<void>(resolve => { release = resolve })
    const controller = new AbortController()
    const next = vi.fn()
    observeRead(async signal => {
      const value = await readObservedDependency(signal, source.dependency)
      if (pause) await wait
      return value
    }, controller.signal, next, error => { throw error })
    await Promise.resolve()
    source.commit({ title: '外部写入', lockVersion: 2 })
    pause = false
    release?.()
    await vi.waitFor(() => expect(next).toHaveBeenCalledWith({ title: '外部写入', lockVersion: 2 }))
    expect(next).toHaveBeenCalledTimes(1)
    controller.abort()
  })
})
