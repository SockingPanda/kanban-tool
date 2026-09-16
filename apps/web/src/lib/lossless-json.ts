import { compactInteger, I64_MIN, U64_MAX } from '../domain/integer'

/** JSON 文本保留大整数为数字 token，字符串继续带引号；不修改 BigInt.prototype。 */
export function stringifyJson(value: unknown, space = 0): string {
  const indent = ' '.repeat(Math.min(10, Math.max(0, space)))
  const ancestors = new Set<object>()
  function write(item: unknown, depth: number, inArray = false): string | undefined {
    if (item === null) return 'null'
    if (typeof item === 'string' || typeof item === 'boolean') return JSON.stringify(item)
    if (typeof item === 'number') {
      if (!Number.isFinite(item) || (Number.isInteger(item) && !Number.isSafeInteger(item))) throw new TypeError('JSON 数值必须有限且不能是已舍入的整数。')
      return Object.is(item, -0) ? '-0' : String(item)
    }
    if (typeof item === 'bigint') {
      if (item < I64_MIN || item > U64_MAX) throw new RangeError('JSON 整数超出 64 位范围。')
      return String(item)
    }
    if (item === undefined) return inArray ? 'null' : undefined
    if (typeof item !== 'object' || item instanceof Uint8Array) throw new TypeError('无法表示为 JSON。')
    if (ancestors.has(item)) throw new TypeError('JSON 不能包含循环引用。')
    ancestors.add(item)
    const array = Array.isArray(item)
    const parts = array
      ? item.map(entry => write(entry, depth + 1, true) ?? 'null')
      : Object.entries(item).flatMap(([key, entry]) => {
        const text = write(entry, depth + 1)
        return text === undefined ? [] : [`${JSON.stringify(key)}:${indent ? ' ' : ''}${text}`]
      })
    ancestors.delete(item)
    const [open, close] = array ? ['[', ']'] : ['{', '}']
    return parts.length === 0 ? `${open}${close}` : indent
      ? `${open}\n${indent.repeat(depth + 1)}${parts.join(`,\n${indent.repeat(depth + 1)}`)}\n${indent.repeat(depth)}${close}`
      : `${open}${parts.join(',')}${close}`
  }
  return write(value, 0) ?? ''
}

/** 先解析整数 token，再构造 JS 值；不依赖新浏览器的 JSON.parse source 扩展。 */
export function parseJson(text: string): unknown {
  let offset = 0
  const fail = (): never => { throw new SyntaxError(`无效 JSON，位置 ${offset}。`) }
  function whitespace() { while (/[\t\n\r ]/.test(text[offset] ?? '') && offset < text.length) offset += 1 }
  function string(): string {
    const start = offset++
    while (offset < text.length) {
      const char = text[offset++]
      if (char === '\\') offset += 1
      else if (char === '"') return JSON.parse(text.slice(start, offset)) as string
    }
    return fail()
  }
  function value(): unknown {
    whitespace()
    const char = text[offset]
    if (char === '"') return string()
    if (char === '[' || char === '{') {
      const array = char === '['
      const result: unknown[] = []
      const entries: [string, unknown][] = []
      const close = array ? ']' : '}'
      offset += 1
      whitespace()
      if (text[offset] !== close) while (true) {
        if (array) result.push(value())
        else {
          whitespace()
          if (text[offset] !== '"') fail()
          const key = string()
          whitespace()
          if (text[offset++] !== ':') fail()
          entries.push([key, value()])
        }
        whitespace()
        if (text[offset] === close) break
        if (text[offset++] !== ',') fail()
      }
      offset += 1
      return array ? result : Object.fromEntries(entries)
    }
    for (const [token, item] of [['null', null], ['true', true], ['false', false]] as const) {
      if (text.startsWith(token, offset)) { offset += token.length; return item }
    }
    const match = /^-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?/.exec(text.slice(offset))
    if (!match) return fail()
    offset += match[0].length
    if (!/[.eE]/.test(match[0]) && match[0] !== '-0') {
      const integer = BigInt(match[0])
      if (integer < I64_MIN || integer > U64_MAX) throw new RangeError('JSON 整数超出 64 位范围。')
      return compactInteger(integer)
    }
    const number = Number(match[0])
    if (!Number.isFinite(number)) return fail()
    return number
  }
  const result = value()
  whitespace()
  if (offset !== text.length) fail()
  return result
}
