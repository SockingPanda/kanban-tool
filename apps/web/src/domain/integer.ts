/** 64 位整数在安全范围内沿用 number，范围外保留 bigint；字符串始终是字符串。 */
export type Integer = number | bigint

export const I64_MIN = -(1n << 63n)
export const I64_MAX = (1n << 63n) - 1n
export const U64_MAX = (1n << 64n) - 1n

export function compactInteger(value: bigint): Integer {
  return value >= BigInt(Number.MIN_SAFE_INTEGER) && value <= BigInt(Number.MAX_SAFE_INTEGER) ? Number(value) : value
}

export function isInteger(value: unknown, unsigned = false): value is Integer {
  return (typeof value === 'bigint' || (typeof value === 'number' && Number.isSafeInteger(value)))
    && value >= (unsigned ? 0n : I64_MIN) && value <= (unsigned ? U64_MAX : I64_MAX)
}

/** 比较不作减法，避免混合 bigint/number 抛错或相邻大整数被舍入。 */
export function compareInteger(left: Integer, right: Integer): number {
  return left < right ? -1 : left > right ? 1 : 0
}

export function addInteger(left: Integer, right: Integer): Integer {
  return compactInteger(BigInt(left) + BigInt(right))
}

/** Date 可表达的毫秒范围小于安全整数范围；超界值由页面显示原始十进制。 */
export function integerDate(value: Integer): Date | null {
  if (value < -8_640_000_000_000_000n || value > 8_640_000_000_000_000n) return null
  const date = new Date(Number(value))
  return Number.isNaN(date.getTime()) ? null : date
}
