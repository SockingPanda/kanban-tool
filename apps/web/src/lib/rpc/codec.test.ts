import { createRequire } from "node:module"
import { create, fromBinary, toBinary } from "@bufbuild/protobuf"
import { describe, expect, test } from "vitest"
import * as common from "../../generated/rpc/kanban/v1/common_pb"
import * as dto from "../../generated/rpc/kanban/v1/dto_pb"
import * as rpc from "../../generated/rpc/kanban/v1/kanban_pb"
import * as codec from "./codec.generated"
import { decodeJson, encodeJson, int64, RpcCodecError, safeNumber } from "./value-codec"

describe("业务 DTO 与 Protobuf 的字段语义", () => {
  test("PATCH 保留未提供、显式清空、空文本和零值", () => {
    const message = codec.encodeUpdateTaskRequest({ method: "UpdateTask", path: { task_id: "t_1" }, input: { description: null, assignee: "", scheduled_at: 0, due_at: null, expected_lock_version: 0 } })
    expect(message.title).toBeUndefined()
    expect(message.description?.change.case).toBe("clear")
    expect(message.assignee?.change).toEqual({ case: "value", value: "" })
    expect(message.scheduledAt?.change).toEqual({ case: "value", value: 0n })
    expect(codec.decodeRpcRequest("UpdateTask", toBinary(rpc.UpdateTaskRequestSchema, message))).toEqual({ path: { task_id: "t_1" }, input: { description: null, assignee: "", scheduled_at: 0, due_at: null, expected_lock_version: 0 } })
    message.description = create(dto.PatchStringSchema)
    expect(() => codec.decodeRpcRequest("UpdateTask", toBinary(rpc.UpdateTaskRequestSchema, message))).toThrow("PATCH")
  })

  test("optional collection 区分缺失和显式空数组", () => {
    const missing = codec.encodeAddTaskLabelRequest({ method: "AddTaskLabel", path: { task_id: "t_1" } })
    const empty = codec.encodeAddTaskLabelRequest({ method: "AddTaskLabel", path: { task_id: "t_1" }, input: { names: [], create_missing: false } })
    expect(missing.names).toBeUndefined()
    expect(empty.names?.items).toEqual([])
    expect(empty.createMissing).toBe(false)
    expect(codec.decodeRpcRequest("AddTaskLabel", toBinary(rpc.AddTaskLabelRequestSchema, empty)).input).toEqual({ names: [], create_missing: false })
  })

  test("数组过滤、原 sort 字符串和显式 false/zero 不被默认值覆盖", () => {
    const message = codec.encodeListTasksRequest({ method: "ListTasks", path: { board: "fixture" }, query: { status: ["ready", "todo"], priority: [0, 3], label: ["后端 API"], sort: "-updated_at", include_archived: false, limit: 0, offset: 0 } })
    expect(message.status).toEqual([dto.DtoApiTaskStatus.READY, dto.DtoApiTaskStatus.TODO])
    expect(message.sort).toBe(dto.DtoTaskReadSort.UPDATED_AT_DESC)
    expect(message.limit).toBe(0n)
    expect(message.offset).toBe(0n)
    expect(message.includeArchived).toBe(false)
    expect(codec.encodeListTasksRequest({ method: "ListTasks", path: { board: "fixture" } }).limit).toBeUndefined()
  })

  test("64 位输入精确保留边界，不接收已失真的 number", () => {
    expect(int64("9223372036854775807")).toBe((1n << 63n) - 1n)
    expect(int64("18446744073709551615", true)).toBe((1n << 64n) - 1n)
    expect(int64(-(1n << 63n))).toBe(-(1n << 63n))
    expect(() => int64(Number.MAX_SAFE_INTEGER + 1)).toThrow("不能舍入")
    expect(() => int64("9223372036854775808")).toThrow("64 位范围")
    expect(() => int64(-1, true)).toThrow("64 位范围")
    const message = codec.encodeUpdateTaskRequest({ method: "UpdateTask", path: { task_id: "t_1" }, input: { expected_lock_version: (1n << 63n) - 1n } })
    expect(fromBinary(rpc.UpdateTaskRequestSchema, toBinary(rpc.UpdateTaskRequestSchema, message)).expectedLockVersion).toBe((1n << 63n) - 1n)
    expect(() => codec.decodeRpcRequest("UpdateTask", toBinary(rpc.UpdateTaskRequestSchema, message))).toThrow("当前页面可精确显示")
    expect(safeNumber(BigInt(Number.MAX_SAFE_INTEGER))).toBe(Number.MAX_SAFE_INTEGER)
  })

  test("自然 metadata 保留 null、集合、非整数和精确 signed/unsigned wire", () => {
    const value = { empty: {}, list: [null, false, "中文", 0, -3, 0.125], nested: { constructor: "值", ["__proto__"]: "普通 key" } }
    expect(decodeJson(fromBinary(common.JsonValueSchema, toBinary(common.JsonValueSchema, encodeJson(value))))).toEqual(value)
    const signed = encodeJson(-(1n << 63n))
    const unsigned = encodeJson((1n << 64n) - 1n)
    expect(signed.kind).toEqual({ case: "signedValue", value: -(1n << 63n) })
    expect(unsigned.kind).toEqual({ case: "unsignedValue", value: (1n << 64n) - 1n })
    expect(() => decodeJson(unsigned)).toThrow("避免数据失真")
    expect(() => encodeJson(Number.MAX_SAFE_INTEGER + 1)).toThrow("不能舍入")
    expect(() => encodeJson(Number.POSITIVE_INFINITY)).toThrow(RpcCodecError)
    expect(() => decodeJson(create(common.JsonValueSchema))).toThrow("值类型")
    expect(Object.is(decodeJson(encodeJson(-0)), -0)).toBe(true)
  })

  test("JsonBodyFieldWire 保留 missing 与 explicit null", () => {
    const input = { actor: { name: "测试", type: "user" }, signals: [] }
    const missing = codec.encodeRecordLabelOntologyObservationRequest({ method: "RecordLabelOntologyObservation", path: { task_id: "t_1" }, input })
    const present = codec.encodeRecordLabelOntologyObservationRequest({ method: "RecordLabelOntologyObservation", path: { task_id: "t_1" }, input: { ...input, agent_candidates: null, final_decision: [], suggest_degraded: false } })
    expect(missing.agentCandidates).toBeUndefined()
    expect(present.agentCandidates?.kind.case).toBe("nullValue")
    expect(present.suggestDegraded).toBe(false)
    expect(codec.decodeRpcRequest("RecordLabelOntologyObservation", toBinary(rpc.RecordLabelOntologyObservationRequestSchema, present)).input).toMatchObject({ agent_candidates: null, final_decision: [], suggest_degraded: false })
  })

  test("ESM 与 CommonJS 的 create/binary map 保留特殊键与多层 null", () => {
    const cjs = createRequire(import.meta.url)("@bufbuild/protobuf") as typeof import("@bufbuild/protobuf")
    const value = { ["__proto__"]: { constructor: [null, { ["__proto__"]: null }] }, constructor: null }
    for (const runtime of [{ create, toBinary, fromBinary }, cjs]) {
      const entries = Object.fromEntries(Object.entries(value).map(([key, item]) => [key, encodeJson(item)]))
      const object = runtime.create(common.JsonObjectSchema, { entries })
      expect(Object.hasOwn(object.entries, "__proto__")).toBe(true)
      expect(Object.getPrototypeOf(object.entries)).toBe(Object.prototype)
      const decoded = runtime.fromBinary(common.JsonObjectSchema, runtime.toBinary(common.JsonObjectSchema, object))
      expect(Object.hasOwn(decoded.entries, "__proto__")).toBe(true)
      expect(Object.getPrototypeOf(decoded.entries)).toBe(Object.prototype)
      expect(decodeJson(create(common.JsonValueSchema, { kind: { case: "objectValue", value: decoded } }))).toEqual(value)
    }
  })

  test("serde rename、枚举和 newtype 校验按真实业务表示转换", () => {
    const dependency = { id: "t_1", board_id: "b_1", board_slug: "test", ref: "test#1", title: "任务", status: "todo" }
    expect(codec.decodeDtoApiDependencyTask(codec.encodeDtoApiDependencyTask(dependency))).toEqual(dependency)
    expect(codec.encodeDtoTaskReadSort("-seq")).toBe(dto.DtoTaskReadSort.SEQ_DESC)
    expect(() => codec.encodeDtoTaskReadSort("seq_desc")).toThrow("枚举值")
    expect(() => codec.decodeDtoApiTaskStatus(999 as dto.DtoApiTaskStatus)).toThrow("枚举数值")
    expect(() => codec.encodeDtoApiTaskPriority(4)).toThrow("允许范围")
    expect(() => codec.decodeDtoApiTaskPriority(create(dto.DtoApiTaskPrioritySchema, { value: 4 }))).toThrow("允许范围")
  })

  test("known EventPayload 使用对应分支，future kind 保留自然值", () => {
    const base = { id: 42, event_id: "e_1", board_id: "b_1", task_id: "t_1", run_id: null, actor: null, created_at: 0 }
    const known = { ...base, kind: "task.created", payload: { status: "todo" } }
    const future = { ...base, kind: "future.mutation", payload: { extra: [null, true] } }
    for (const value of [known, future]) {
      const message = codec.encodeDtoStreamEventData(value)
      expect(codec.decodeDtoStreamEventData(fromBinary(dto.DtoStreamEventDataSchema, toBinary(dto.DtoStreamEventDataSchema, message)))).toEqual(value)
    }
    const wrong = codec.encodeDtoStreamEventData(known)
    wrong.payload = create(dto.DtoEventPayloadSchema, { value: { case: "unknown", value: encodeJson(known.payload) } })
    expect(() => codec.decodeDtoStreamEventData(wrong)).toThrow("分支不一致")
    expect(() => codec.encodeDtoStreamEventData({ ...base, kind: "task.step.done", payload: { status: "todo" } })).toThrow("状态不一致")
    expect(() => codec.encodeDtoStreamEventData({ ...known, task_id: null })).toThrow("task_id")
  })

  test("附件上传和下载直接持有 bytes，完整元数据保持不变", () => {
    const content = new Uint8Array([0, 255, 128, 10])
    const upload = codec.encodeCreateAttachmentRequest({ method: "CreateAttachment", path: { task_id: "t_1" }, input: { filename: "报告.bin", content } })
    expect(upload.content).toBe(content)
    const attachment = { id: "a_1", board_id: "b_1", task_id: "t_1", filename: "报告.bin", rel_path: "a_1", content_type: "application/octet-stream", size_bytes: 4, sha256: "hash", created_by: "测试", created_at: 0 }
    const wire = codec.encodeDownloadAttachmentResponse({ attachment, content })
    expect(wire.content).toBe(content)
    const decoded = codec.decodeDownloadAttachmentResponse(wire)
    expect(decoded.content).toBe(content)
    expect(decoded.attachment).toEqual(attachment)
    expect(() => codec.encodeCreateAttachmentRequest({ method: "CreateAttachment", path: { task_id: "t_1" }, input: { filename: "bad", content: [1, 2] } })).toThrow("Uint8Array")
  })

  test("逆向接口生成真实 response bytes，拒绝未声明的业务字段", () => {
    const bytes = codec.encodeRpcResponse("ListBoards", { data: [] })
    expect(fromBinary(dto.ListBoardsResponseSchema, bytes).data).toEqual([])
    expect(() => codec.encodeListBoardsRequest({ method: "ListBoards", query: { execute: "anything" } })).toThrow("未声明字段")
  })
})
