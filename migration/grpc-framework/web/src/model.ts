/** 前端内部投影模型。wire 解码仅由生成的 Protobuf client 完成。 */
export type Card = Readonly<{
  id: string; title: string; status: number; priority: number;
  position: bigint; seq: bigint; lockVersion: bigint;
}>
export type Cursor = Readonly<{ epoch: string; scope: string; revision: bigint }>
export type View = Readonly<{ boardId: string; cursor: Cursor; cards: ReadonlyMap<string, Card> }>
export type Body =
  | { kind: "begin"; revision: bigint; count: number }
  | { kind: "chunk"; index: number; tasks: readonly Card[] }
  | { kind: "commit"; revision: bigint; count: number; chunks: number }
  | { kind: "delta"; base: bigint; revision: bigint; upserts: readonly Card[]; removed: readonly string[] }
  | { kind: "heartbeat"; serverRevision: bigint }
  | { kind: "reset"; reason: string }
export type Frame = Readonly<{ boardId: string; epoch: string; scope: string; body: Body }>
export class ProtocolError extends Error {}
export const U64_MAX = 18446744073709551615n
export function revision(value: bigint): void {
  if (typeof value !== "bigint" || value < 0n || value > U64_MAX) throw new ProtocolError("无效的 uint64")
}
export function cardWeight(card: Card): number {
  return new TextEncoder().encode(card.id + card.title).byteLength + 128
}
export function validateCard(card: Card): void {
  if (!card.id.startsWith("t_") || card.id.length <= 2 || new TextEncoder().encode(card.id).byteLength > 128
    || /[\u0000-\u001f\u007f-\u009f]/.test(card.id) || card.title.trim() === "" || new TextEncoder().encode(card.title).byteLength > 4096
    || !Number.isInteger(card.status) || card.status < 1 || card.status > 9
    || !Number.isInteger(card.priority) || card.priority < 0 || card.priority > 3
    || typeof card.position !== "bigint" || card.position < -9223372036854775808n || card.position > 9223372036854775807n) {
    throw new ProtocolError("无效的 TaskCard")
  }
  revision(card.seq); revision(card.lockVersion)
}
export function orderedCards(view: View): readonly Card[] {
  return [...view.cards.values()].sort((a, b) => {
    for (const [left, right] of [[a.position, b.position], [a.seq, b.seq]] as const) {
      if (left !== right) return left < right ? -1 : 1
    }
    return a.id < b.id ? -1 : a.id > b.id ? 1 : 0
  })
}
