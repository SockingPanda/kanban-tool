import type { WebRuntimeConfig } from "../runtime"
import { parseApiExplainLabelAtomResponse } from "./generated/contracts/api-explain-label-atom-response"
import type { ApiExplainLabelAtomResponseContract } from "./generated/contracts/api-explain-label-atom-response"
import {
  parseApiGetLabelOntologySignalPath,
} from "./generated/contracts/api-get-label-ontology-signal-path"
import {
  parseApiGetLabelOntologySignalResponse,
} from "./generated/contracts/api-get-label-ontology-signal-response"
import type { ApiGetLabelOntologySignalResponseContract } from "./generated/contracts/api-get-label-ontology-signal-response"
import { parseApiGetSignalPath } from "./generated/contracts/api-get-signal-path"
import { parseApiGetSignalResponse } from "./generated/contracts/api-get-signal-response"
import type { ApiGetSignalResponseContract } from "./generated/contracts/api-get-signal-response"
import { parseApiLabelAtomPath } from "./generated/contracts/api-label-atom-path"
import { parseApiLabelOntologyReviewQuery } from "./generated/contracts/api-label-ontology-review-query"
import { parseApiLabelOntologySignalQuery } from "./generated/contracts/api-label-ontology-signal-query"
import { parseApiListLabelOntologySignalsPath } from "./generated/contracts/api-list-label-ontology-signals-path"
import {
  parseApiListLabelOntologySignalsResponse,
} from "./generated/contracts/api-list-label-ontology-signals-response"
import type { ApiListLabelOntologySignalsResponseContract } from "./generated/contracts/api-list-label-ontology-signals-response"
import { parseApiReviewLabelOntologyPath } from "./generated/contracts/api-review-label-ontology-path"
import { parseApiReviewLabelOntologyResponse } from "./generated/contracts/api-review-label-ontology-response"
import type { ApiReviewLabelOntologyResponseContract } from "./generated/contracts/api-review-label-ontology-response"
import { parseApiReviewSignalsPath } from "./generated/contracts/api-review-signals-path"
import { parseApiReviewSignalsQuery } from "./generated/contracts/api-review-signals-query"
import { parseApiReviewSignalsResponse } from "./generated/contracts/api-review-signals-response"
import type { ApiReviewSignalsResponseContract } from "./generated/contracts/api-review-signals-response"
import { createHttpTransport, type HttpTransport, type HttpTransportOptions } from "./http-transport"

export type SignalRecord = ApiReviewSignalsResponseContract["data"][number]
export type LabelOntologySignalRecord = ApiListLabelOntologySignalsResponseContract["data"][number]
export type LabelOntologySignalDetail = ApiGetLabelOntologySignalResponseContract["data"]
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

export interface SignalsOntologyReadTransport extends HttpTransport {}

export interface SignalsOntologyReadApi {
  readonly board: string
  readonly actor: string
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
}

function encodedSegment(value: string): string {
  return encodeURIComponent(value)
}

function appendArray(params: URLSearchParams, key: string, values: readonly string[]): void {
  for (const value of values) params.append(key, value)
}

function signalListQuery(query: SignalListQuery | undefined) {
  const parsed = parseApiReviewSignalsQuery({
    status: [...(query?.statuses ?? [])],
    kind: [...(query?.kinds ?? [])],
    task_ref: query?.task?.trim() || null,
    include_all: query?.includeAll ?? false,
    limit: query?.limit ?? 100,
  })
  const params = new URLSearchParams()
  appendArray(params, "status", parsed.status ?? [])
  appendArray(params, "kind", parsed.kind ?? [])
  if (parsed.task_ref !== null && parsed.task_ref !== undefined) params.set("task_ref", parsed.task_ref)
  params.set("include_all", String(parsed.include_all ?? false))
  params.set("limit", String(parsed.limit ?? 100))
  return params
}

function ontologySignalQuery(query: OntologySignalListQuery | undefined) {
  const parsed = parseApiLabelOntologySignalQuery({
    status: [...(query?.statuses ?? [])],
    kind: [...(query?.kinds ?? [])],
    task_ref: query?.task?.trim() || null,
    target_label_ref: query?.targetLabelRef?.trim() || null,
    proposed_label_name: query?.proposedLabelName?.trim() || null,
    include_all: query?.includeAll ?? false,
    limit: query?.limit ?? 100,
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
    limit: query?.limit ?? 100,
  })
  const params = new URLSearchParams({
    group_by: parsed.group_by ?? "label",
    include_all: String(parsed.include_all ?? false),
    limit: String(parsed.limit ?? 100),
  })
  return params
}

function reviewSignalsPath(board: string): string {
  const parsed = parseApiReviewSignalsPath({ board })
  return `/api/v1/boards/${encodedSegment(parsed.board)}/signals/review`
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

export function createSignalsOntologyReadApi(
  runtime: WebRuntimeConfig,
  options: SignalsOntologyReadModelOptions,
): SignalsOntologyReadApi {
  const transport = options.transport ?? createHttpTransport(runtime, options)
  const board = options.board

  return {
    board,
    actor: runtime.actor,
    async reviewSignals(query, signal) {
      const response = await transport.get(`${reviewSignalsPath(board)}?${signalListQuery(query).toString()}`, signal)
      return parseApiReviewSignalsResponse(response.payload).data
    },
    async getSignal(signalId, signal) {
      const response = await transport.get(signalPath(signalId), signal)
      return parseApiGetSignalResponse(response.payload).data
    },
    async listLabelOntologySignals(query, signal) {
      const response = await transport.get(`${ontologySignalsPath(board)}?${ontologySignalQuery(query).toString()}`, signal)
      return parseApiListLabelOntologySignalsResponse(response.payload).data
    },
    async reviewLabelOntology(query, signal) {
      const response = await transport.get(`${ontologyReviewPath(board)}?${ontologyReviewQuery(query).toString()}`, signal)
      return parseApiReviewLabelOntologyResponse(response.payload).data
    },
    async getLabelOntologySignal(signalId, signal) {
      const response = await transport.get(ontologySignalPath(signalId), signal)
      return parseApiGetLabelOntologySignalResponse(response.payload).data
    },
    async explainLabelAtom(atomRef, signal) {
      const response = await transport.get(atomExplainPath(board, atomRef), signal)
      return parseApiExplainLabelAtomResponse(response.payload).data
    },
  }
}
