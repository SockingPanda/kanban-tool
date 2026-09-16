import { create, toBinary } from "@bufbuild/protobuf"
import type { RpcCall } from "../../application/data/rpc-transport"
import { QueryDefinitionSchema, type QueryDefinition, type QueryCursor, type QueryResult } from "../../generated/rpc/kanban/v1/query_pb"
import * as codec from "./codec.generated"
import { RpcCodecError, int32, record, text } from "./value-codec"

export interface RecentEventsCall {
  readonly method: "RecentEvents"
  readonly query: { readonly board: string; readonly task_id?: string; readonly limit: number }
  readonly signal?: AbortSignal
}
export type QueryCall = RpcCall | RecentEventsCall
const queryMethods = new Set<string>(["ListBoards", "ListBoardColumns", "ListTasks", "ListTasksByStatus", "GetTask", "ListTaskLabels", "ListDependencies", "ListSteps", "ListComments", "ListAttachments", "TaskNeighborhood", "BoardTaskMap", "ListRuns", "GetRun", "GetRunLog", "ListEvents", "GetStats", "GetHealth", "MaintenanceStatus", "SearchStatus", "GetTaskDetails", "GetBoard", "ListBoardLabels", "SearchTasks", "SearchTasksByStatus", "RecentEvents"])

export function isQueryCall(call: QueryCall): boolean { return queryMethods.has(call.method) }

/** 复用正式具名请求 codec；refresh、订阅 ID 和 cursor 都不属于查询身份。 */
export function encodeQueryDefinition(call: QueryCall, clientQueryId: string, resume?: QueryCursor, refresh = false): QueryDefinition {
  let query: QueryDefinition["query"]
  switch (call.method) {
    case "ListBoards": query = { case: "listBoards", value: codec.encodeListBoardsRequest(call) }; break
    case "ListBoardColumns": query = { case: "listBoardColumns", value: codec.encodeListBoardColumnsRequest(call) }; break
    case "ListTasks": query = { case: "listTasks", value: codec.encodeListTasksRequest(call) }; break
    case "ListTasksByStatus": query = { case: "listTasksByStatus", value: codec.encodeListTasksByStatusRequest(call) }; break
    case "GetTask": query = { case: "getTask", value: codec.encodeGetTaskRequest(call) }; break
    case "ListTaskLabels": query = { case: "listTaskLabels", value: codec.encodeListTaskLabelsRequest(call) }; break
    case "ListDependencies": query = { case: "listDependencies", value: codec.encodeListDependenciesRequest(call) }; break
    case "ListSteps": query = { case: "listSteps", value: codec.encodeListStepsRequest(call) }; break
    case "ListComments": query = { case: "listComments", value: codec.encodeListCommentsRequest(call) }; break
    case "ListAttachments": query = { case: "listAttachments", value: codec.encodeListAttachmentsRequest(call) }; break
    case "TaskNeighborhood": query = { case: "taskNeighborhood", value: codec.encodeTaskNeighborhoodRequest(call) }; break
    case "BoardTaskMap": query = { case: "boardTaskMap", value: codec.encodeBoardTaskMapRequest(call) }; break
    case "ListRuns": query = { case: "listRuns", value: codec.encodeListRunsRequest(call) }; break
    case "GetRun": query = { case: "getRun", value: codec.encodeGetRunRequest(call) }; break
    case "GetRunLog": query = { case: "getRunLog", value: codec.encodeGetRunLogRequest(call) }; break
    case "ListEvents": query = { case: "listEvents", value: codec.encodeListEventsRequest(call) }; break
    case "GetStats": query = { case: "getStats", value: codec.encodeGetStatsRequest(call) }; break
    case "GetHealth": query = { case: "getHealth", value: codec.encodeGetHealthRequest(call) }; break
    case "MaintenanceStatus": query = { case: "maintenanceStatus", value: codec.encodeMaintenanceStatusRequest(call) }; break
    case "SearchStatus": query = { case: "searchStatus", value: codec.encodeSearchStatusRequest(call) }; break
    case "GetTaskDetails": query = { case: "getTaskDetails", value: codec.encodeGetTaskDetailsRequest(call) }; break
    case "GetBoard": query = { case: "getBoard", value: codec.encodeGetBoardRequest(call) }; break
    case "ListBoardLabels": query = { case: "listBoardLabels", value: codec.encodeListBoardLabelsRequest(call) }; break
    case "SearchTasks": query = { case: "searchTasks", value: codec.encodeSearchTasksRequest(call) }; break
    case "SearchTasksByStatus": query = { case: "searchTasksByStatus", value: codec.encodeSearchTasksByStatusRequest(call) }; break
    case "RecentEvents": {
      const value = record(call.query, ["board", "task_id", "limit"])
      const limit = int32(value.limit, true, 1000)
      if (limit < 1) throw new RpcCodecError("最近事件窗口 limit 必须大于 0。")
      query = { case: "recentEvents", value: { $typeName: "kanban.v1.RecentEventsQuery", boardId: text(value.board), taskId: value.task_id === undefined ? undefined : text(value.task_id), limit } }
      break
    }
    default: throw new RpcCodecError("此操作没有持续查询投影。")
  }
  return create(QueryDefinitionSchema, { clientQueryId, projectionVersion: 1, resume, refresh, query })
}

/** 本地身份使用完整请求编码；服务端独立解析 canonical board 并计算共享 scope。 */
export function queryIdentity(call: QueryCall): string {
  return Array.from(toBinary(QueryDefinitionSchema, encodeQueryDefinition(call, "")), byte => byte.toString(16).padStart(2, "0")).join("")
}

export function decodeQueryResult(definition: QueryDefinition, projection: QueryResult): unknown {
  const result = projection.result
  if (!definition.query.case || result.case !== definition.query.case) throw new RpcCodecError("查询投影类型与请求不一致。")
  switch (result.case) {
    case "listBoards": return codec.decodeListBoardsResponse(result.value)
    case "listBoardColumns": return codec.decodeListBoardColumnsResponse(result.value)
    case "listTasks": return codec.decodeListTasksResponse(result.value)
    case "listTasksByStatus": return codec.decodeListTasksByStatusResponse(result.value)
    case "getTask": return codec.decodeGetTaskResponse(result.value)
    case "listTaskLabels": return codec.decodeListTaskLabelsResponse(result.value)
    case "listDependencies": return codec.decodeListDependenciesResponse(result.value)
    case "listSteps": return codec.decodeListStepsResponse(result.value)
    case "listComments": return codec.decodeListCommentsResponse(result.value)
    case "listAttachments": return codec.decodeListAttachmentsResponse(result.value)
    case "taskNeighborhood": return codec.decodeTaskNeighborhoodResponse(result.value)
    case "boardTaskMap": return codec.decodeBoardTaskMapResponse(result.value)
    case "listRuns": return codec.decodeListRunsResponse(result.value)
    case "getRun": return codec.decodeGetRunResponse(result.value)
    case "getRunLog": return codec.decodeGetRunLogResponse(result.value)
    case "listEvents": return codec.decodeListEventsResponse(result.value)
    case "getStats": return codec.decodeGetStatsResponse(result.value)
    case "getHealth": return codec.decodeGetHealthResponse(result.value)
    case "maintenanceStatus": return codec.decodeMaintenanceStatusResponse(result.value)
    case "searchStatus": return codec.decodeSearchStatusResponse(result.value)
    case "getTaskDetails": return codec.decodeGetTaskDetailsResponse(result.value)
    case "getBoard": return codec.decodeGetBoardResponse(result.value)
    case "listBoardLabels": return codec.decodeListBoardLabelsResponse(result.value)
    case "searchTasks": return codec.decodeSearchTasksResponse(result.value)
    case "searchTasksByStatus": return codec.decodeSearchTasksByStatusResponse(result.value)
    case "recentEvents": return codec.decodeListEventsResponse(result.value)
    default: throw new RpcCodecError("查询投影缺少结果。")
  }
}
