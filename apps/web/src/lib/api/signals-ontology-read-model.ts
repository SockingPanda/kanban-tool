import type { CanonicalBoardId } from "../sync/contracts"
import { asCanonicalBoardId } from "../sync/contracts"
import { parseCanonicalBoardSlug } from "../board-slug"
import type { WebRuntimeConfig } from "../runtime"
import { parseApiExplainLabelAtomResponse } from "./generated/contracts/api-explain-label-atom-response"
import type { ApiExplainLabelAtomResponseContract } from "./generated/contracts/api-explain-label-atom-response"
import { parseApiGetLabelOntologySignalPath } from "./generated/contracts/api-get-label-ontology-signal-path"
import { parseApiGetLabelOntologySignalResponse } from "./generated/contracts/api-get-label-ontology-signal-response"
import type { ApiGetLabelOntologySignalResponseContract } from "./generated/contracts/api-get-label-ontology-signal-response"
import { parseApiGetSignalPath } from "./generated/contracts/api-get-signal-path"
import { parseApiGetSignalResponse } from "./generated/contracts/api-get-signal-response"
import { parseApiLabelAtomPath } from "./generated/contracts/api-label-atom-path"
import { parseApiLabelOntologyReviewQuery } from "./generated/contracts/api-label-ontology-review-query"
import { parseApiLabelOntologySignalQuery } from "./generated/contracts/api-label-ontology-signal-query"
import { parseApiListBoardsQuery } from "./generated/contracts/api-list-boards-query"
import { parseApiListBoardsResponse } from "./generated/contracts/api-list-boards-response"
import type { ApiListBoardsResponseContract } from "./generated/contracts/api-list-boards-response"
import { parseApiListLabelOntologySignalsPath } from "./generated/contracts/api-list-label-ontology-signals-path"
import { parseApiListLabelOntologySignalsResponse } from "./generated/contracts/api-list-label-ontology-signals-response"
import type { ApiListLabelOntologySignalsResponseContract } from "./generated/contracts/api-list-label-ontology-signals-response"
import { parseApiReviewLabelOntologyPath } from "./generated/contracts/api-review-label-ontology-path"
import { parseApiReviewLabelOntologyResponse } from "./generated/contracts/api-review-label-ontology-response"
import type { ApiReviewLabelOntologyResponseContract } from "./generated/contracts/api-review-label-ontology-response"
import { parseApiReviewSignalsPath } from "./generated/contracts/api-review-signals-path"
import { parseApiReviewSignalsQuery } from "./generated/contracts/api-review-signals-query"
import { parseApiReviewSignalsResponse } from "./generated/contracts/api-review-signals-response"
import type { ApiReviewSignalsResponseContract } from "./generated/contracts/api-review-signals-response"
import { ContractValidationError } from "./generated/runtime"
import { createHttpTransport, HttpTransportError, type HttpTransport, type HttpTransportOptions, type HttpTransportResponse } from "./http-transport"

export type SignalRecord = ApiReviewSignalsResponseContract["data"][number]
export type LabelOntologySignalRecord = ApiListLabelOntologySignalsResponseContract["data"][number]
export type LabelOntologySignalDetail = ApiGetLabelOntologySignalResponseContract["data"]
export type LabelOntologyActionRecord = LabelOntologySignalDetail["actions"][number]
export type LabelOntologyReviewGroup = ApiReviewLabelOntologyResponseContract["data"][number]
export type LabelAtomExplainRecord = ApiExplainLabelAtomResponseContract["data"]

export interface SignalListQuery {
  readonly statuses?: readonly string[]
  readonly kinds?: readonly string[]
  readonly task?: string
  readonly includeAll?: boolean
  readonly limit?: number
}

export interface OntologySignalListQuery extends SignalListQuery {
  readonly targetLabelRef?: string
  readonly proposedLabelName?: string
}

export interface OntologyReviewQuery {
  readonly groupBy?: "label" | "candidate_atom" | "proposed_label" | "cluster"
  readonly includeAll?: boolean
  readonly limit?: number
}

export interface FeatureBoardIdentity {
  readonly selector: string
  readonly canonicalBoardId: CanonicalBoardId
  readonly slug: string
  readonly name: string
}

export class SignalsOntologyReadError extends Error {
  readonly kind: "identity" | "invalid_response" | "board_scope" | "http"
  readonly status: number | null
  readonly contractId: string | null

  constructor(
    kind: SignalsOntologyReadError["kind"],
    message: string,
    options: { status?: number; contractId?: string; cause?: unknown } = {},
  ) {
    super(message, { cause: options.cause })
    this.name = "SignalsOntologyReadError"
    this.kind = kind
    this.status = options.status ?? null
    this.contractId = options.contractId ?? null
  }
}

export type SignalsOntologyReadTransport = HttpTransport

export interface SignalsOntologyReadApi {
  /** URL selector used for diagnostics; requests use the resolved canonical b_ ID. */
  readonly board: string
  readonly actor: string
  readonly identity?: FeatureBoardIdentity
  readonly canonicalBoardId?: CanonicalBoardId
  readonly cacheKey: string
  resolveIdentity(signal?: AbortSignal): Promise<FeatureBoardIdentity>
  reviewSignals(query?: SignalListQuery, signal?: AbortSignal): Promise<readonly SignalRecord[]>
  getSignal(signalId: string, signal?: AbortSignal): Promise<SignalRecord>
  listLabelOntologySignals(query?: OntologySignalListQuery, signal?: AbortSignal): Promise<readonly LabelOntologySignalRecord[]>
  reviewLabelOntology(query?: OntologyReviewQuery, signal?: AbortSignal): Promise<readonly LabelOntologyReviewGroup[]>
  getLabelOntologySignal(signalId: string, signal?: AbortSignal): Promise<LabelOntologySignalDetail>
  explainLabelAtom(atomRef: string, signal?: AbortSignal): Promise<LabelAtomExplainRecord>
}

export interface SignalsOntologyReadModelOptions extends HttpTransportOptions {
  readonly board: string
  readonly transport?: SignalsOntologyReadTransport
  readonly identity?: FeatureBoardIdentity
}

const RENDERED_SIGNAL_STATUSES = ["open", "confirmed", "resolved", "rejected", "superseded"] as const

function encodedSegment(value: string): string {
  return encodeURIComponent(value)
}

function appendArray(params: URLSearchParams, key: string, values: readonly string[]): void {
  for (const value of values) params.append(key, value)
}

function requestedLimit(value: number | undefined): number {
  const limit = value ?? 100
  if (!Number.isSafeInteger(limit) || limit < 0 || limit > 100) {
    throw new SignalsOntologyReadError("invalid_response", "请求 limit 必须是 0 到 100 之间的安全整数。")
  }
  return limit
}

function signalQuery(query: SignalListQuery | undefined, includeAll: boolean, statuses: readonly string[]) {
  const parsed = parseApiReviewSignalsQuery({
    status: [...statuses],
    kind: [...(query?.kinds ?? [])],
    task_ref: query?.task?.trim() || null,
    include_all: includeAll,
    limit: requestedLimit(query?.limit),
  })
  const params = new URLSearchParams()
  appendArray(params, "status", parsed.status ?? [])
  appendArray(params, "kind", parsed.kind ?? [])
  if (parsed.task_ref !== null && parsed.task_ref !== undefined) params.set("task_ref", parsed.task_ref)
  params.set("include_all", String(parsed.include_all ?? false))
  params.set("limit", String(parsed.limit ?? 100))
  return params
}

function ontologySignalQuery(query: OntologySignalListQuery | undefined, includeAll: boolean, statuses: readonly string[]) {
  const parsed = parseApiLabelOntologySignalQuery({
    status: [...statuses],
    kind: [...(query?.kinds ?? [])],
    task_ref: query?.task?.trim() || null,
    target_label_ref: query?.targetLabelRef?.trim() || null,
    proposed_label_name: query?.proposedLabelName?.trim() || null,
    include_all: includeAll,
    limit: requestedLimit(query?.limit),
  })
  const params = new URLSearchParams()
  appendArray(params, "status", parsed.status ?? [])
  appendArray(params, "kind", parsed.kind ?? [])
  if (parsed.task_ref !== null && parsed.task_ref !== undefined) params.set("task_ref", parsed.task_ref)
  if (parsed.target_label_ref !== null && parsed.target_label_ref !== undefined) params.set("target_label_ref", parsed.target_label_ref)
  if (parsed.proposed_label_name !== null && parsed.proposed_label_name !== undefined) params.set("proposed_label_name", parsed.proposed_label_name)
  params.set("include_all", String(parsed.include_all ?? false))
  params.set("limit", String(parsed.limit ?? 100))
  return params
}

function ontologyReviewQuery(query: OntologyReviewQuery | undefined) {
  const parsed = parseApiLabelOntologyReviewQuery({
    group_by: query?.groupBy ?? "label",
    include_all: query?.includeAll ?? false,
    limit: requestedLimit(query?.limit),
  })
  return new URLSearchParams({
    group_by: parsed.group_by ?? "label",
    include_all: String(parsed.include_all ?? false),
    limit: String(parsed.limit ?? 100),
  })
}

function listBoardsQuery(): string {
  const parsed = parseApiListBoardsQuery({ include_archived: false })
  return `/api/v1/boards?include_archived=${String(parsed.include_archived ?? false)}`
}

function reviewSignalsPath(board: string, review: boolean): string {
  const parsed = parseApiReviewSignalsPath({ board })
  return `/api/v1/boards/${encodedSegment(parsed.board)}/signals${review ? "/review" : ""}`
}

function signalPath(signalId: string): string {
  const parsed = parseApiGetSignalPath({ signal_id: signalId })
  return `/api/v1/signals/${encodedSegment(parsed.signal_id)}`
}

function ontologySignalsPath(board: string): string {
  const parsed = parseApiListLabelOntologySignalsPath({ board })
  return `/api/v1/boards/${encodedSegment(parsed.board)}/label-ontology/signals`
}

function ontologyReviewPath(board: string): string {
  const parsed = parseApiReviewLabelOntologyPath({ board })
  return `/api/v1/boards/${encodedSegment(parsed.board)}/label-ontology/review`
}

function ontologySignalPath(signalId: string): string {
  const parsed = parseApiGetLabelOntologySignalPath({ signal_id: signalId })
  return `/api/v1/label-ontology/signals/${encodedSegment(parsed.signal_id)}`
}

function atomExplainPath(board: string, atomRef: string): string {
  const parsed = parseApiLabelAtomPath({ board, atom_ref: atomRef })
  return `/api/v1/boards/${encodedSegment(parsed.board)}/labels/atoms/${encodedSegment(parsed.atom_ref)}/explain`
}

function isPrintable(value: string): boolean {
  return value.length > 0 && [...value].every((character) => {
    const code = character.codePointAt(0) ?? 0
    return code > 0x1f && !(code >= 0x7f && code <= 0x9f)
  })
}

function validateBoardList(boards: ApiListBoardsResponseContract["data"], selector: string): FeatureBoardIdentity {
  const ids = new Set<string>()
  const slugs = new Set<string>()
  for (const board of boards) {
    if (!board.id.startsWith("b_") || !isPrintable(board.id) || parseCanonicalBoardSlug(board.slug) === null || board.name.trim().length === 0) {
      throw new SignalsOntologyReadError("identity", "看板列表包含无效 canonical identity。", { cause: { selector } })
    }
    if (ids.has(board.id) || slugs.has(board.slug)) {
      throw new SignalsOntologyReadError("identity", "看板列表包含重复 id 或 slug。", { cause: { selector } })
    }
    ids.add(board.id)
    slugs.add(board.slug)
  }
  const matches = boards.filter((board) => board.id === selector || board.slug === selector)
  if (matches.length !== 1 || matches[0] === undefined) {
    throw new SignalsOntologyReadError("identity", "请求的看板 selector 未精确匹配一个 canonical board。", { cause: { selector } })
  }
  const board = matches[0]
  return Object.freeze({ selector, canonicalBoardId: asCanonicalBoardId(board.id), slug: board.slug, name: board.name.trim() })
}

async function readPayload(transport: SignalsOntologyReadTransport, path: string, signal?: AbortSignal): Promise<HttpTransportResponse> {
  try {
    return await transport.get(path, signal)
  } catch (error) {
    if (error instanceof SignalsOntologyReadError) throw error
    if (error instanceof HttpTransportError) {
      throw new SignalsOntologyReadError("http", error.message, { status: error.status ?? undefined, cause: error })
    }
    throw error
  }
}

function parseResponse<T>(contractId: string, parser: (value: unknown) => T, payload: unknown): T {
  try {
    return parser(payload)
  } catch (error) {
    if (error instanceof ContractValidationError) {
      throw new SignalsOntologyReadError("invalid_response", `Web API 响应不符合 ${contractId} contract。`, { contractId, cause: error })
    }
    throw error
  }
}

function assertMeta(meta: { readonly include_all: boolean; readonly limit: number }, includeAll: boolean, limit: number, count: number): void {
  if (meta.include_all !== includeAll || meta.limit !== limit || count > limit) {
    throw new SignalsOntologyReadError("invalid_response", "Web API 响应 meta 与请求或 data 数组不一致。")
  }
}

function assertBoardId(value: string, expected: CanonicalBoardId, field: string): void {
  if (value !== expected) throw new SignalsOntologyReadError("board_scope", `${field} 返回了错误的 board_id。`)
}

function assertSignalBoard(signal: SignalRecord, boardId: CanonicalBoardId): void {
  assertBoardId(signal.board_id, boardId, "signal")
  assertBoardId(signal.observation.board_id, boardId, "signal.observation")
}

function assertOntologySignalBoard(signal: LabelOntologySignalRecord, boardId: CanonicalBoardId, field = "ontology.signal"): void {
  assertBoardId(signal.board_id, boardId, field)
}

function assertOntologyDetailBoard(detail: LabelOntologySignalDetail, boardId: CanonicalBoardId): void {
  assertOntologySignalBoard(detail.signal, boardId, "ontology.detail.signal")
  assertBoardId(detail.observation.board_id, boardId, "ontology.detail.observation")
  for (const signal of detail.observation.signals) assertOntologySignalBoard(signal, boardId, "ontology.detail.observation.signal")
  for (const action of detail.actions) assertBoardId(action.board_id, boardId, "ontology.detail.action")
}

function assertAtomBoard(explain: LabelAtomExplainRecord, boardId: CanonicalBoardId): void {
  if (explain.atom) assertBoardId(explain.atom.board_id, boardId, "atom")
  if (explain.current_semantics) assertBoardId(explain.current_semantics.board_id, boardId, "atom.current_semantics")
  for (const entry of explain.provenance_actions) assertBoardId(entry.action.board_id, boardId, "atom.provenance_action")
  for (const entry of explain.validation_history) assertBoardId(entry.action.board_id, boardId, "atom.validation_action")
  for (const entry of explain.supporting_signals) {
    assertOntologySignalBoard(entry.signal, boardId, "atom.supporting_signal")
    assertBoardId(entry.observation.board_id, boardId, "atom.supporting_observation")
    assertBoardId(entry.source_task.board_id, boardId, "atom.source_task")
  }
}

function mergeAndSort<T extends { readonly id: string; readonly created_at: number }>(rows: readonly T[], limit: number): readonly T[] {
  const byId = new Map<string, T>()
  for (const row of rows) if (!byId.has(row.id)) byId.set(row.id, row)
  return [...byId.values()]
    .sort((left, right) => right.created_at - left.created_at || left.id.localeCompare(right.id))
    .slice(0, limit)
}

export async function resolveSignalsOntologyBoardIdentity(
  runtime: WebRuntimeConfig,
  selector: string,
  options: { readonly transport?: SignalsOntologyReadTransport; readonly fetcher?: typeof fetch; readonly documentBaseURI?: string; readonly signal?: AbortSignal } = {},
): Promise<FeatureBoardIdentity> {
  const transport = options.transport ?? createHttpTransport(runtime, options)
  const response = await readPayload(transport, listBoardsQuery(), options.signal)
  const payload = parseResponse("api.list-boards.response", parseApiListBoardsResponse, response.payload)
  return validateBoardList(payload.data, selector.trim())
}

export function createSignalsOntologyReadApi(runtime: WebRuntimeConfig, options: SignalsOntologyReadModelOptions): SignalsOntologyReadApi {
  const transport = options.transport ?? createHttpTransport(runtime, options)
  const selector = options.board.trim()
  let cachedIdentity = options.identity
  let identityPromise: Promise<FeatureBoardIdentity> | null = cachedIdentity ? Promise.resolve(cachedIdentity) : null
  const resolveIdentity = (signal?: AbortSignal): Promise<FeatureBoardIdentity> => {
    if (cachedIdentity) return Promise.resolve(cachedIdentity)
    if (identityPromise) return identityPromise
    identityPromise = resolveSignalsOntologyBoardIdentity(runtime, selector, { transport, signal }).then((identity) => {
      cachedIdentity = identity
      return identity
    }).catch((error) => {
      identityPromise = null
      throw error
    })
    return identityPromise
  }

  const api: SignalsOntologyReadApi = {
    board: selector,
    actor: runtime.actor,
    get identity() { return cachedIdentity },
    get canonicalBoardId() { return cachedIdentity?.canonicalBoardId },
    get cacheKey() { return `${runtime.webBuildId}:${runtime.serverVersion}:${runtime.apiBaseUrl}:${cachedIdentity?.canonicalBoardId ?? selector}` },
    resolveIdentity,
    async reviewSignals(query, signal) {
      const identity = await resolveIdentity(signal)
      const limit = requestedLimit(query?.limit)
      const statuses = [...(query?.statuses ?? [])].filter(Boolean)
      const includeAll = query?.includeAll === true
      const calls = includeAll
        ? (statuses.length > 0 ? statuses : [...RENDERED_SIGNAL_STATUSES]).map(async (status) => {
            const params = signalQuery(query, true, [status])
            const response = await readPayload(transport, `${reviewSignalsPath(identity.canonicalBoardId, false)}?${params}`, signal)
            const parsed = parseResponse("api.review-signals.response", parseApiReviewSignalsResponse, response.payload)
            assertMeta(parsed.meta, true, limit, parsed.data.length)
            for (const row of parsed.data) assertSignalBoard(row, identity.canonicalBoardId)
            return parsed.data
          })
        : [readPayload(transport, `${reviewSignalsPath(identity.canonicalBoardId, true)}?${signalQuery(query, false, statuses)}`, signal).then((response) => {
            const parsed = parseResponse("api.review-signals.response", parseApiReviewSignalsResponse, response.payload)
            assertMeta(parsed.meta, false, limit, parsed.data.length)
            for (const row of parsed.data) assertSignalBoard(row, identity.canonicalBoardId)
            return parsed.data
          })]
      const rows = (await Promise.all(calls)).flat()
      return mergeAndSort(rows, limit)
    },
    async getSignal(signalId, signal) {
      const identity = await resolveIdentity(signal)
      const response = await readPayload(transport, signalPath(signalId), signal)
      const signalRecord = parseResponse("api.get-signal.response", parseApiGetSignalResponse, response.payload).data
      assertSignalBoard(signalRecord, identity.canonicalBoardId)
      return signalRecord
    },
    async listLabelOntologySignals(query, signal) {
      const identity = await resolveIdentity(signal)
      const limit = requestedLimit(query?.limit)
      const statuses = [...(query?.statuses ?? [])].filter(Boolean)
      const includeAll = query?.includeAll === true
      const calls = includeAll && statuses.length === 0
        ? RENDERED_SIGNAL_STATUSES.map((status) => [status])
        : [statuses]
      const rows = (await Promise.all(calls.map(async (statusSet) => {
        const params = ontologySignalQuery(query, includeAll, statusSet)
        const response = await readPayload(transport, `${ontologySignalsPath(identity.canonicalBoardId)}?${params}`, signal)
        const parsed = parseResponse("api.list-label-ontology-signals.response", parseApiListLabelOntologySignalsResponse, response.payload)
        assertMeta(parsed.meta, includeAll, limit, parsed.data.length)
        for (const row of parsed.data) assertOntologySignalBoard(row, identity.canonicalBoardId)
        return parsed.data
      }))).flat()
      return mergeAndSort(rows, limit)
    },
    async reviewLabelOntology(query, signal) {
      const identity = await resolveIdentity(signal)
      const limit = requestedLimit(query?.limit)
      const groupBy = query?.groupBy ?? "label"
      const includeAll = query?.includeAll === true
      const params = ontologyReviewQuery(query)
      const response = await readPayload(transport, `${ontologyReviewPath(identity.canonicalBoardId)}?${params}`, signal)
      const parsed = parseResponse("api.review-label-ontology.response", parseApiReviewLabelOntologyResponse, response.payload)
      if (parsed.meta.group_by !== groupBy || parsed.meta.include_all !== includeAll || parsed.meta.limit !== limit || parsed.data.length > limit) {
        throw new SignalsOntologyReadError("invalid_response", "Ontology review response meta 与请求或 data 数组不一致。")
      }
      return parsed.data
    },
    async getLabelOntologySignal(signalId, signal) {
      const identity = await resolveIdentity(signal)
      const response = await readPayload(transport, ontologySignalPath(signalId), signal)
      const detail = parseResponse("api.get-label-ontology-signal.response", parseApiGetLabelOntologySignalResponse, response.payload).data
      assertOntologyDetailBoard(detail, identity.canonicalBoardId)
      return detail
    },
    async explainLabelAtom(atomRef, signal) {
      const identity = await resolveIdentity(signal)
      const response = await readPayload(transport, atomExplainPath(identity.canonicalBoardId, atomRef), signal)
      const explain = parseResponse("api.explain-label-atom.response", parseApiExplainLabelAtomResponse, response.payload).data
      assertAtomBoard(explain, identity.canonicalBoardId)
      return explain
    },
  }
  return api
}
