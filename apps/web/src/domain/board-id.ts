/** 看板查询返回的 canonical boards.id，与 URL 中使用的 slug/selector 区分。 */
declare const canonicalBoardIdBrand: unique symbol
export type CanonicalBoardId = string & { readonly [canonicalBoardIdBrand]: "CanonicalBoardId" }
export function asCanonicalBoardId(value: string): CanonicalBoardId {
  if (value.length === 0) throw new RangeError("canonical board ID 不得为空")
  return value as CanonicalBoardId
}
