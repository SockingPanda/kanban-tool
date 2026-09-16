import { ProtocolError, cardWeight, validateCard, revision, type Card, type Cursor, type Frame, type View } from "./model.js"

type Pending = { cursor: Cursor; expected: number; chunks: number; bytes: number; cards: Map<string, Card> }
export class ProjectionStore {
  private current: View | undefined
  private pending: Pending | undefined
  private forceSnapshot = false
  private lastDelta: string | undefined
  readonly scope: string
  constructor(
    readonly boardId: string,
    private readonly publish: (view: View) => void,
    private readonly limits = { maxCards: 50_000, maxBytes: 16 * 1024 * 1024 },
  ) {
    if (!/^b_[^\u0000-\u001f\u007f-\u009f]+$/.test(boardId) || new TextEncoder().encode(boardId).byteLength > 128) throw new ProtocolError("无效 board ID")
    if (!Number.isSafeInteger(limits.maxCards) || limits.maxCards <= 0 || !Number.isSafeInteger(limits.maxBytes) || limits.maxBytes <= 0) throw new ProtocolError("无效预算")
    this.scope = `board:${boardId}:cards:v1`
  }
  snapshot(): View | undefined { return this.current }
  resume(): Cursor | undefined { return this.forceSnapshot ? undefined : this.current?.cursor }
  discardPending(): void { this.pending = undefined }
  requireSnapshot(): void { this.pending = undefined; this.forceSnapshot = true }
  private count(value: number): void {
    if (!Number.isSafeInteger(value) || value < 0 || value > this.limits.maxCards) throw new ProtocolError("快照数量无效或超过预算")
  }
  private identity(frame: Frame, cursor: Cursor): boolean { return frame.epoch === cursor.epoch && frame.scope === cursor.scope }
  private budget(cards: ReadonlyMap<string, Card>): void {
    this.count(cards.size)
    let bytes = 0
    for (const card of cards.values()) {
      bytes += cardWeight(card)
      if (bytes > this.limits.maxBytes) throw new ProtocolError("投影超出字节预算")
    }
  }
  private commit(view: View): void {
    // 回调必须是同步、原子的状态替换；成功返回后才能确认 cursor。
    this.publish(view)
    this.current = view
    this.pending = undefined
    this.forceSnapshot = false
  }
  apply(frame: Frame): void {
    if (frame.boardId !== this.boardId || frame.scope !== this.scope || frame.epoch.length === 0 || frame.epoch.length > 128) {
      throw new ProtocolError("订阅身份不匹配")
    }
    const body = frame.body
    switch (body.kind) {
      case "reset":
        this.requireSnapshot()
        return
      case "begin": {
        revision(body.revision); this.count(body.count)
        if (this.pending !== undefined || body.revision === 0n) throw new ProtocolError("重复或无效 snapshot begin")
        if (this.current && this.identity(frame, this.current.cursor) && body.revision < this.current.cursor.revision) throw new ProtocolError("快照版本回退")
        if (this.current && !this.identity(frame, this.current.cursor) && !this.forceSnapshot) throw new ProtocolError("epoch 改变但未 reset")
        this.pending = { cursor: { epoch: frame.epoch, scope: frame.scope, revision: body.revision }, expected: body.count, chunks: 0, bytes: 0, cards: new Map() }
        return
      }
      case "chunk": {
        const pending = this.pending
        if (!pending || !this.identity(frame, pending.cursor) || body.index !== pending.chunks || body.tasks.length === 0 || body.tasks.length > 64) throw new ProtocolError("快照片段缺失、越序或越界")
        // 先完整验证当前片段，再改变 staging。
        const ids = new Set<string>()
        let bytes = pending.bytes
        for (const card of body.tasks) {
          validateCard(card)
          if (ids.has(card.id) || pending.cards.has(card.id)) throw new ProtocolError("快照包含重复任务")
          ids.add(card.id); bytes += cardWeight(card)
        }
        if (pending.cards.size + ids.size > pending.expected || bytes > this.limits.maxBytes) throw new ProtocolError("快照超出声明或预算")
        for (const card of body.tasks) pending.cards.set(card.id, Object.freeze({ ...card }))
        pending.bytes = bytes; pending.chunks += 1
        return
      }
      case "commit": {
        const pending = this.pending
        revision(body.revision)
        if (!pending || !this.identity(frame, pending.cursor) || body.revision !== pending.cursor.revision
          || body.count !== pending.expected || body.count !== pending.cards.size || body.chunks !== pending.chunks) throw new ProtocolError("snapshot commit 不完整")
        this.commit(Object.freeze({ boardId: this.boardId, cursor: Object.freeze(pending.cursor), cards: pending.cards }))
        this.lastDelta = undefined
        return
      }
      case "heartbeat":
        revision(body.serverRevision)
        if (this.current && !this.forceSnapshot && !this.identity(frame, this.current.cursor)) throw new ProtocolError("心跳 epoch 不匹配")
        return // 收到数据的活性不等于已应用数据的位置。
      case "delta": {
        const current = this.current
        revision(body.base); revision(body.revision)
        if (!current || this.pending || this.forceSnapshot || !this.identity(frame, current.cursor)) throw new ProtocolError("delta 没有可用的已提交基线")
        const fingerprint = JSON.stringify(body, (_key, value: unknown) => typeof value === "bigint" ? value.toString() : value)
        if (body.revision === current.cursor.revision && fingerprint === this.lastDelta) return
        if (body.base !== current.cursor.revision || body.revision !== body.base + 1n) throw new ProtocolError("增量版本缺失或冲突")
        const seen = new Set<string>()
        const cards = new Map(current.cards)
        for (const id of body.removed) {
          if (seen.has(id) || !cards.has(id)) throw new ProtocolError("重复或未知 remove")
          seen.add(id); cards.delete(id)
        }
        for (const card of body.upserts) {
          validateCard(card)
          if (seen.has(card.id)) throw new ProtocolError("同批任务重复或 remove/upsert 冲突")
          seen.add(card.id)
          const old = cards.get(card.id)
          if (old && card.lockVersion < old.lockVersion) throw new ProtocolError("任务 lock version 回退")
          cards.set(card.id, Object.freeze({ ...card }))
        }
        this.budget(cards)
        this.commit(Object.freeze({ boardId: this.boardId, cursor: Object.freeze({ ...current.cursor, revision: body.revision }), cards }))
        this.lastDelta = fingerprint
        return
      }
      default:
        throw new ProtocolError("未知的必需流消息")
    }
  }
}
