import { describe, expect, test } from 'vitest'
import { I64_MIN, I64_MAX, U64_MAX } from '../domain/integer'
import { parseJson, stringifyJson } from './lossless-json'
import { decodeJson, encodeJson } from './rpc/value-codec'

describe('动态 JSON 的数字身份', () => {
  test('文本、嵌套 metadata 和 RPC 往返保留整数 token 与字符串的区别', () => {
    const source = '{"min":-9223372036854775808,"max":9223372036854775807,"unsigned":18446744073709551615,"text":"18446744073709551615","safe":9007199254740991,"next":9007199254740992,"nested":[null,true,0.125,-0,{"__proto__":9223372036854775807}]}'
    const value = parseJson(source)
    expect(value).toEqual({ min: I64_MIN, max: I64_MAX, unsigned: U64_MAX, text: String(U64_MAX), safe: Number.MAX_SAFE_INTEGER, next: 9007199254740992n, nested: [null, true, 0.125, -0, { ['__proto__']: I64_MAX }] })
    expect(stringifyJson(value)).toBe(source)
    expect(parseJson(stringifyJson(value, 2))).toEqual(value)
    expect(decodeJson(encodeJson(value))).toEqual(value)
    expect(stringifyJson({ value: U64_MAX })).not.toBe(stringifyJson({ value: String(U64_MAX) }))
    expect(stringifyJson({ value: 9007199254740992n })).not.toBe(stringifyJson({ value: 9007199254740993n }))
  })

  test('安全边界两侧使用规范表示，JSON 语法与范围错误不默默修复', () => {
    for (const value of [I64_MIN, -9007199254740992n, -9007199254740991n, 0n, 9007199254740991n, 9007199254740992n, I64_MAX, U64_MAX]) {
      const parsed = parseJson(String(value))
      expect(BigInt(String(parsed))).toBe(value)
      expect(typeof parsed).toBe(value >= -9007199254740991n && value <= 9007199254740991n ? 'number' : 'bigint')
    }
    for (const source of ['01', '[1,]', '{"x":1,}', 'true false', '"unterminated', '1e999', String(I64_MIN - 1n), String(U64_MAX + 1n)]) expect(() => parseJson(source)).toThrow()
    expect(() => stringifyJson(Number.MAX_SAFE_INTEGER + 1)).toThrow('舍入')
    expect(() => stringifyJson(U64_MAX + 1n)).toThrow('64 位')
    expect(() => stringifyJson({ n: Number.NaN })).toThrow()
  })
})
