import { create } from "@bufbuild/protobuf"
import { EmptySchema, JsonArraySchema, JsonObjectSchema, JsonValueSchema, type JsonValue } from "../../generated/rpc/kanban/v1/common_pb"

/** 只表示字段转换错误；transport 负责转换为页面的稳定错误类型。 */
export class RpcCodecError extends Error {
  constructor(message: string) { super(message); this.name = "RpcCodecError" }
}

export function required<T>(value: T | undefined | null, field = "字段"): T {
  if (value === undefined || value === null) throw new RpcCodecError(`RPC ${field} 缺少必填值。`)
  return value
}
export function record(value: unknown, keys?: readonly string[]): Record<string, unknown> {
  if (value === null || typeof value !== "object" || Array.isArray(value) || value instanceof Uint8Array) throw new RpcCodecError("RPC 字段必须是对象。")
  if (keys !== undefined) {
    const allowed = new Set(keys)
    if (Object.keys(value).some((key) => !allowed.has(key))) throw new RpcCodecError("RPC 业务对象包含未声明字段。")
  }
  return value as Record<string, unknown>
}
export function omitUndefined(value: Record<string, unknown>): Record<string, unknown> {
  return Object.fromEntries(Object.entries(value).filter(([, value]) => value !== undefined))
}
export function decodePatch<T extends { case: "clear" | "value" | undefined }, U>(
  change: T | undefined,
  convert: (change: Extract<T, { case: "value" }>) => U,
): U | null | undefined {
  if (change === undefined) return undefined
  if (change.case === "clear") return null
  if (change.case === "value") return convert(change as Extract<T, { case: "value" }>)
  throw new RpcCodecError("RPC PATCH 必须明确赋值或清空。")
}
export function text(value: unknown): string {
  if (typeof value !== "string") throw new RpcCodecError("RPC 字段必须是字符串。")
  return value
}
export function bool(value: unknown): boolean {
  if (typeof value !== "boolean") throw new RpcCodecError("RPC 字段必须是布尔值。")
  return value
}
export function float(value: unknown, single = false): number {
  if (typeof value !== "number" || !Number.isFinite(value) || (single && !Number.isFinite(Math.fround(value)))) throw new RpcCodecError("RPC 字段必须是有限数值。")
  return single ? Math.fround(value) : value
}
export function int32(value: unknown, unsigned = false, maximum = unsigned ? 0xffffffff : 0x7fffffff): number {
  if (typeof value !== "number" || !Number.isInteger(value) || value < (unsigned ? 0 : -0x80000000) || value > maximum) throw new RpcCodecError("RPC 整数字段超出允许范围。")
  return value
}
export function int64(value: unknown, unsigned = false): bigint {
  let integer: bigint
  if (typeof value === "bigint") integer = value
  else if (typeof value === "number" && Number.isSafeInteger(value)) integer = BigInt(value)
  else if (typeof value === "string" && /^-?(?:0|[1-9][0-9]*)$/.test(value)) integer = BigInt(value)
  else throw new RpcCodecError("RPC 64 位整数必须是安全整数、bigint 或精确十进制字符串；不能舍入。")
  if (integer < (unsigned ? 0n : -(1n << 63n)) || integer > (unsigned ? (1n << 64n) - 1n : (1n << 63n) - 1n)) throw new RpcCodecError("RPC 整数字段超出 64 位范围。")
  return integer
}
export function safeNumber(value: bigint): number {
  if (value < BigInt(Number.MIN_SAFE_INTEGER) || value > BigInt(Number.MAX_SAFE_INTEGER)) throw new RpcCodecError("服务器返回的整数超出当前页面可精确显示的范围；为避免数据失真，已停止转换。")
  return Number(value)
}
export function taskPriority(value: unknown): number { return int32(value, true, 3) }
export function unitInterval(value: unknown): number {
  const number = float(value)
  if (number < 0 || number > 1) throw new RpcCodecError("RPC 比例字段必须在 0 到 1 之间。")
  return number
}
export function positiveRank(value: unknown): unknown {
  if (int64(value) < 1n) throw new RpcCodecError("RPC 排名必须是正整数。")
  return value
}
export function taskReadLabel(value: unknown): string {
  const label = text(value).trim()
  if (label.length === 0 || [...text(value)].length > 128) throw new RpcCodecError("RPC 标签必须包含非空白字符，且最多 128 个 Unicode 字符。")
  return label
}
export function bytes(value: unknown): Uint8Array {
  if (!(value instanceof Uint8Array)) throw new RpcCodecError("RPC 附件内容必须是 Uint8Array。")
  return value
}
export function array<T>(value: unknown, convert: (value: unknown) => T): T[] {
  if (!Array.isArray(value)) throw new RpcCodecError("RPC 字段必须是数组。")
  return value.map(convert)
}
export function dictionary<T>(value: unknown, convert: (value: unknown) => T): Record<string, T> {
  return Object.fromEntries(Object.entries(record(value)).map(([key, item]) => [key, convert(item)]))
}
export function optional<T>(value: unknown, convert: (value: unknown) => T): T | undefined {
  return value === undefined || value === null ? undefined : convert(value)
}
export function present<T>(value: unknown, convert: (value: unknown) => T): T | undefined {
  return value === undefined ? undefined : convert(value)
}
export function nullable<T, U>(value: T | undefined, convert: (value: T) => U): U | null {
  return value === undefined ? null : convert(value)
}
export function enumValue<T extends number>(value: unknown, names: Readonly<Record<string, T>>): T {
  const name = text(value)
  if (!Object.hasOwn(names, name)) throw new RpcCodecError(`RPC 枚举值 ${name} 无效。`)
  return names[name]
}
export function enumName(value: number, names: Readonly<Record<string, number>>): string {
  const entry = Object.entries(names).find(([, number]) => number === value)
  if (entry === undefined) throw new RpcCodecError(`RPC 枚举数值 ${value} 未定义。`)
  return entry[0]
}
export function encodeJson(value: unknown): JsonValue {
  if (value === null) return create(JsonValueSchema, { kind: { case: "nullValue", value: create(EmptySchema) } })
  if (typeof value === "boolean") return create(JsonValueSchema, { kind: { case: "boolValue", value } })
  if (typeof value === "string") return create(JsonValueSchema, { kind: { case: "stringValue", value } })
  if (typeof value === "number" || typeof value === "bigint") {
    if (typeof value === "number" && (!Number.isInteger(value) || Object.is(value, -0))) return create(JsonValueSchema, { kind: { case: "floatValue", value: float(value) } })
    const signed = value < 0
    const integer = int64(value, !signed)
    return create(JsonValueSchema, { kind: signed ? { case: "signedValue", value: integer } : { case: "unsignedValue", value: integer } })
  }
  if (Array.isArray(value)) return create(JsonValueSchema, { kind: { case: "arrayValue", value: create(JsonArraySchema, { items: value.map(encodeJson) }) } })
  if (typeof value === "object" && value !== null && !(value instanceof Uint8Array)) return create(JsonValueSchema, { kind: { case: "objectValue", value: create(JsonObjectSchema, { entries: dictionary(value, encodeJson) }) } })
  throw new RpcCodecError("RPC metadata/result/evidence 包含无法表示的值。")
}
export function decodeJson(message: JsonValue): unknown {
  switch (message.kind.case) {
    case "nullValue": return null
    case "boolValue": case "stringValue": return message.kind.value
    case "signedValue": case "unsignedValue": return safeNumber(message.kind.value)
    case "floatValue": return float(message.kind.value)
    case "arrayValue": return message.kind.value.items.map(decodeJson)
    case "objectValue": return Object.fromEntries(Object.entries(message.kind.value.entries).map(([key, item]) => [key, decodeJson(item)]))
    default: throw new RpcCodecError("RPC 动态字段没有指定值类型。")
  }
}
