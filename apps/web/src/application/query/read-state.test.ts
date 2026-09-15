import { expect, test } from "vitest"

import { asyncReadToken, visibleAsyncReadState } from "./read-state"

test("Health 重试保留原错误区；切换 identity 立即隐藏旧错误", () => {
  const error = new Error("服务暂时不可用")
  const state = { ...asyncReadToken(true, "host-a", 0), data: null, error, loading: false }
  expect(visibleAsyncReadState(state, asyncReadToken(true, "host-a", 1), true, true)).toEqual({ data: null, error, loading: true })
  expect(visibleAsyncReadState(state, asyncReadToken(true, "host-b", 0), true, true)).toEqual({ data: null, error: null, loading: true })
  expect(visibleAsyncReadState(state, asyncReadToken(true, "host-a", 1), true)).toEqual({ data: null, error: null, loading: true })
})
