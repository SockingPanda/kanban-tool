import { describe, expect, it } from "vitest"

import { ApiError } from "./errors"
import { errorMessage, presentApiError } from "./error-presentation"

describe("API error presentation", () => {
  it("turns network failures into an actionable server hint and keeps the raw error", () => {
    const message = presentApiError(new TypeError("Failed to fetch"))

    expect(message).toContain("服务不可用，请先启动或检查 kanban serve。")
    expect(message).toContain("原始错误：Failed to fetch")
  })

  it("keeps degraded capability reasons and structured details visible", () => {
    const message = presentApiError(new ApiError("degraded", "vector store disabled", { capability: "vector32" }))

    expect(message).toContain("服务返回了降级结果，请检查能力原因。")
    expect(message).toContain("degraded: vector store disabled")
    expect(message).toContain('details={"capability":"vector32"}')
  })

  it("localizes invalid and unavailable capability responses", () => {
    expect(errorMessage(new ApiError("feature_not_available", "attachments route missing"), "zh-CN")).toContain("服务未提供请求的能力。")
    expect(errorMessage(new ApiError("invalid_response", "response must be valid JSON"), "zh-CN")).toContain("桌面客户端收到无效服务响应")
  })

  it("does not present restart-required replacement as a successful mutation", () => {
    const message = errorMessage(new ApiError("restart_required", "portable replacement staged"), "zh-CN")

    expect(message).toContain("操作未应用")
    expect(message).toContain("重启 kanban serve")
    expect(message).toContain("restart_required: portable replacement staged")
  })
})
