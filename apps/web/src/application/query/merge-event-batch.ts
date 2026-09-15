import { ExplorerReadError, mergeBoardEvents, type BoardEventsBatch, type BoardEventsReadModel } from '../data/explorer-read-model';

type EventState = {identityKey:string;data:BoardEventsReadModel|null;error:Error|null;stale:boolean};
/** 校验增量事件并合并快照；保持项目隔离、事件次序和单调游标。 */
export function mergeEventBatch<T extends EventState>(current:T,batch:BoardEventsBatch,identityKey:string,taskId:string|null):T {
      if (current.identityKey !== identityKey || !current.data || current.data.board.id !== batch.boardId || (taskId !== null && current.data.taskId !== taskId)) return current
      try {
        if (!Number.isSafeInteger(batch.nextAfter) || batch.nextAfter < 0) {
          throw new ExplorerReadError("anomaly", "事件 batch 的 nextAfter 不是非负安全整数。")
        }
        let previousId = -1
        let maxIncomingId = -1
        for (const event of batch.events) {
          if (!Number.isSafeInteger(event.id) || event.id <= 0 || event.id <= previousId) {
            throw new ExplorerReadError("anomaly", "事件 batch 的 id 必须严格递增。")
          }
          if (event.board_id !== batch.boardId) {
            throw new ExplorerReadError("anomaly", "事件 batch 越过当前 board scope。")
          }
          if (event.event_id.trim().length === 0) {
            throw new ExplorerReadError("anomaly", "事件 batch 缺少 event_id。")
          }
          if (event.id > batch.nextAfter) {
            throw new ExplorerReadError("anomaly", "事件 batch 的 id 不得超过 nextAfter。")
          }
          previousId = event.id
          maxIncomingId = event.id
        }
        // 初次读取已包含的迟到 batch 不得回退游标。
        if (batch.nextAfter <= current.data.meta.nextAfter) return current
        if (batch.events.length === 0) {
          if (batch.nextAfter !== current.data.meta.nextAfter) {
            throw new ExplorerReadError("anomaly", "空事件 batch 不得推进 nextAfter。")
          }
        } else if (batch.nextAfter !== maxIncomingId) {
          throw new ExplorerReadError("anomaly", "事件 batch 的 nextAfter 必须等于最后一个事件 id。")
        }
        // 初次读取结束后重新应用期间收到的 batch。
        const incoming = taskId === null ? batch.events : batch.events.filter((event) => event.task_id === taskId)
        const events = mergeBoardEvents(current.data.events, incoming, batch.boardId)


        return {
          ...current,
          data: { ...current.data, events, meta: { ...current.data.meta, count: events.length, nextAfter: Math.max(current.data.meta.nextAfter, batch.nextAfter) } },
          error: null,
          stale: false,
        }
      } catch (error) {
        return { ...current, error: error instanceof Error ? error : new Error(String(error)), stale: true }
      }
}
