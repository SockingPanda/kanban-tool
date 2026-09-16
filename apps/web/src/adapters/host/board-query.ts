import { observeRead } from '../../application/query/observe-read'
import type { BoardReadModel, BoardReadModelOptions, BoardReadQuery } from '../../application/data/board-read-model'
import type { WebRuntimeConfig } from '../../lib/runtime'
import type { QueryRegistry } from './query-registry'
import { loadBoardReadModel } from './board-read-model'

/** 会话的展示映射复用共享查询；显式 reload 通过 Ready barrier 等待服务端新读取。 */
export function createSubscribedBoardQuery(runtime: WebRuntimeConfig, selector: string | undefined, options: BoardReadModelOptions, registry: QueryRegistry): BoardReadQuery {
  const controllers = new Set<AbortController>()
  const once = (signal: AbortSignal | undefined, refresh: boolean): Promise<BoardReadModel> => {
    const controller = new AbortController()
    controllers.add(controller)
    const abort = () => controller.abort(signal?.reason)
    signal?.addEventListener('abort', abort, { once: true })
    if (signal?.aborted) abort()
    return new Promise<BoardReadModel>((resolve, reject) => {
      if (controller.signal.aborted) { reject(controller.signal.reason); return }
      controller.signal.addEventListener('abort', () => reject(controller.signal.reason), { once: true })
      observeRead(nextSignal => loadBoardReadModel(runtime, selector, { ...options, signal: nextSignal }), controller.signal, resolve, reject, refresh)
    }).finally(() => { signal?.removeEventListener('abort', abort); controllers.delete(controller); controller.abort() })
  }
  return {
    load: signal => once(signal, false),
    reload: signal => once(signal, true),
    invalidate: () => { for (const controller of controllers) controller.abort(); controllers.clear() },
    observe: (signal, next, failed) => observeRead(nextSignal => loadBoardReadModel(runtime, selector, { ...options, signal: nextSignal }), signal, next, failed),
    subscribeConnection: listener => registry.subscribeState(listener),
  }
}
