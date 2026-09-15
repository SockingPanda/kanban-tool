import { create, fromBinary, toBinary } from "@bufbuild/protobuf"
import { describe, expect, test } from "vitest"
import { ListEventsResponseSchema, ListTasksResponseSchema } from "../../generated/rpc/kanban/v1/dto_pb"
import { QueryCursorSchema, QueryDefinitionSchema, QueryResultSchema } from "../../generated/rpc/kanban/v1/query_pb"
import { encodeRpcResponse } from "./codec.generated"
import { decodeQueryResult, encodeQueryDefinition, isQueryCall, queryIdentity, type QueryCall } from "./query-codec"

const list: QueryCall = { method: "ListTasks", path: { board: "b_demo" }, query: { limit: 25, offset: 0, sort: "-updated_at", status: ["ready", "todo"] } }

describe("完整查询与具名业务契约的映射", () => {
  test("身份包含完整过滤、排序和分页，排除本地对象顺序与取消信号", () => {
    expect(queryIdentity(list)).toBe(queryIdentity({ ...list, query: { status: ["ready", "todo"], sort: "-updated_at", offset: 0, limit: 25 }, signal: new AbortController().signal }))
    for (const query of [{ limit: 25, offset: 25 }, { limit: 50, offset: 0 }, { sort: "title" }, { status: ["done"] }]) {
      expect(queryIdentity({ ...list, query })).not.toBe(queryIdentity(list))
    }
    expect(queryIdentity({ ...list, path: { board: "b_other" } })).not.toBe(queryIdentity(list))
  })

  test("恢复 revision 保持64位，显式刷新不改变查询body", () => {
    const cursor = create(QueryCursorSchema, { epoch: "epoch", scope: "scope", revision: (1n << 64n) - 1n })
    const resumed = encodeQueryDefinition(list, "subscriber-2", cursor, true)
    const decoded = fromBinary(QueryDefinitionSchema, toBinary(QueryDefinitionSchema, resumed))
    expect(decoded.resume?.revision).toBe((1n << 64n) - 1n)
    expect(decoded.refresh).toBe(true)
    expect(decoded.query).toEqual(encodeQueryDefinition(list, "subscriber-1").query)
  })

  test("完整列表投影保留total和空分页，不接受其他query结果", () => {
    const payload = { data: [], meta: { total: 37, limit: 25, offset: 50 } }
    const response = fromBinary(ListTasksResponseSchema, encodeRpcResponse("ListTasks", payload))
    const projection = create(QueryResultSchema, { result: { case: "listTasks", value: response } })
    expect(decodeQueryResult(encodeQueryDefinition(list, "list"), projection)).toEqual(payload)
    const other = encodeQueryDefinition({ method: "GetTask", path: { task_id: "t_one" } }, "detail")
    expect(() => decodeQueryResult(other, projection)).toThrow("不一致")
  })

  test("RecentEvents复用完整事件契约并拒绝无限窗口", () => {
    const call: QueryCall = { method: "RecentEvents", query: { board: "b_demo", task_id: "t_1", limit: 150 } }
    const definition = encodeQueryDefinition(call, "events")
    expect(definition.query.case).toBe("recentEvents")
    const payload = { data: [], meta: { next_after: 42 } }
    const response = fromBinary(ListEventsResponseSchema, encodeRpcResponse("ListEvents", payload))
    expect(decodeQueryResult(definition, create(QueryResultSchema, { result: { case: "recentEvents", value: response } }))).toEqual(payload)
    for (const limit of [0, -1, 1001, Number.MAX_SAFE_INTEGER]) {
      expect(() => encodeQueryDefinition({ ...call, query: { board: "b_demo", limit } }, "events")).toThrow()
    }
  })

  test("写命令不能进入查询服务", () => {
    const mutation: QueryCall = { method: "CreateTask", path: { board: "b_demo" }, input: { title: "任务" } }
    expect(isQueryCall(mutation)).toBe(false)
    expect(() => encodeQueryDefinition(mutation, "write")).toThrow("没有持续查询")
    expect(isQueryCall(list)).toBe(true)
  })
})
