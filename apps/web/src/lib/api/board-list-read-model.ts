import { asCanonicalBoardId, type CanonicalBoardId } from "../sync/contracts"
import { parseCanonicalBoardSlug, type CanonicalBoardSlug } from "../board-slug"
import type { WebRuntimeConfig } from "../runtime"
import { parseApiListBoardsQuery } from "./generated/contracts/api-list-boards-query"
import { parseApiListBoardsResponse } from "./generated/contracts/api-list-boards-response"
import type { ApiListBoardsResponseContract } from "./generated/contracts/api-list-boards-response"
import { ContractValidationError } from "./generated/runtime"
import {
  createHttpTransport,
  HttpTransportError,
  type HttpReadTransport,
  type HttpTransportOptions,
  type HttpTransportResponse,
} from "./http-transport"

export type BoardListItem = Readonly<{
  readonly id: CanonicalBoardId
  readonly slug: CanonicalBoardSlug
  readonly name: string
  readonly description: string | null
  readonly archivedAt: number | null
}>

export type BoardListReadErrorKind =
  | "offline"
  | "http"
  | "invalid_json"
  | "invalid_contract"
  | "anomaly"
  | "cross_origin"
  | "malformed_url"
  | "invalid_headers"
  | "invalid_content_type"
  | "response_too_large"

export class BoardListReadError extends Error {
  readonly kind: BoardListReadErrorKind
  readonly status: number | null
  readonly contractId: string | null

  constructor(
    kind: BoardListReadErrorKind,
    message: string,
    options: { readonly status?: number; readonly contractId?: string; readonly cause?: unknown } = {},
  ) {
    super(message, { cause: options.cause })
    this.name = "BoardListReadError"
    this.kind = kind
    this.status = options.status ?? null
    this.contractId = options.contractId ?? null
  }
}

export interface BoardListReadDependencies extends HttpTransportOptions {
  readonly transport?: Pick<HttpReadTransport, "get">
}

export interface BoardListReadOptions extends BoardListReadDependencies {
  readonly signal?: AbortSignal
  readonly includeArchived?: boolean
}

export interface BoardListQueryOptions extends BoardListReadDependencies {
  readonly includeArchived?: boolean
}

export interface BoardListReadQuery {
  load(): Promise<readonly BoardListItem[]>
  reload(): Promise<readonly BoardListItem[]>
  invalidate(): void
}

function isAbortError(error: unknown): boolean {
  return error instanceof Error && error.name === "AbortError"
}

function generationAbortError(): Error {
  const error = new Error("board list read generation 已被取消。")
  error.name = "AbortError"
  return error
}

function wrapTransportError(error: unknown): never {
  if (isAbortError(error)) throw error
  if (error instanceof BoardListReadError) throw error
  if (error instanceof HttpTransportError) {
    throw new BoardListReadError(
      error.kind === "invalid_bytes" ? "anomaly" : error.kind,
      error.message,
      { status: error.status ?? undefined, cause: error },
    )
  }
  throw error
}

function parseContract<T>(contractId: string, parser: (value: unknown) => T, value: unknown): T {
  try {
    return parser(value)
  } catch (error) {
    if (error instanceof ContractValidationError) {
      throw new BoardListReadError(
        "invalid_contract",
        `Web API 响应不符合 ${contractId} contract。`,
        { contractId, cause: error },
      )
    }
    throw error
  }
}

function boardListPath(includeArchived: boolean): string {
  const query = parseApiListBoardsQuery({ include_archived: includeArchived })
  const params = new URLSearchParams()
  params.set("include_archived", String(query.include_archived))
  return `/api/v1/boards?${params.toString()}`
}

async function getPayload(
  transport: Pick<HttpReadTransport, "get">,
  path: string,
  signal: AbortSignal | undefined,
): Promise<unknown> {
  try {
    const response: HttpTransportResponse = await transport.get(path, signal)
    if (
      response === null
      || typeof response !== "object"
      || !("payload" in response)
      || typeof response.bytes !== "number"
      || !Number.isSafeInteger(response.bytes)
      || response.bytes < 0
    ) {
      throw new BoardListReadError("anomaly", "Web API transport 返回了无效响应。")
    }
    return response.payload
  } catch (error) {
    return wrapTransportError(error)
  }
}

function validCanonicalBoardId(value: string): CanonicalBoardId | null {
  if (!/^b_[A-Za-z0-9][A-Za-z0-9._-]*$/.test(value)) return null
  for (const character of value) {
    const codePoint = character.codePointAt(0) ?? 0
    if (codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f)) return null
  }
  try {
    return asCanonicalBoardId(value)
  } catch {
    return null
  }
}

function parseBoardList(boards: ApiListBoardsResponseContract["data"]): readonly BoardListItem[] {
  const selectors = new Set<string>()
  const items = boards.map((candidate) => {
    const id = validCanonicalBoardId(candidate.id)
    const slug = parseCanonicalBoardSlug(candidate.slug)
    const name = candidate.name.trim()
    if (id === null || slug === null || name.length === 0) {
      throw new BoardListReadError("anomaly", "看板列表包含无效 canonical identity。")
    }
    if (selectors.has(id) || selectors.has(slug)) {
      throw new BoardListReadError("anomaly", "看板列表包含重复 canonical identity。")
    }
    selectors.add(id)
    selectors.add(slug)
    return Object.freeze({
      id,
      slug,
      name,
      description: candidate.description,
      archivedAt: candidate.archived_at,
    })
  })
  return Object.freeze(items)
}

export async function loadBoardList(
  runtime: WebRuntimeConfig,
  options: BoardListReadOptions = {},
): Promise<readonly BoardListItem[]> {
  let transport: Pick<HttpReadTransport, "get">
  try {
    transport = options.transport ?? createHttpTransport(runtime, options)
  } catch (error) {
    return wrapTransportError(error)
  }

  try {
    const response = parseContract(
      "api.list-boards.response",
      parseApiListBoardsResponse,
      await getPayload(transport, boardListPath(options.includeArchived ?? false), options.signal),
    )
    return parseBoardList(response.data)
  } catch (error) {
    return wrapTransportError(error)
  }
}

export function createBoardListQuery(
  runtime: WebRuntimeConfig,
  options: BoardListQueryOptions = {},
): BoardListReadQuery {
  let generation = 0
  let cached: readonly BoardListItem[] | null = null
  let generationController: AbortController | null = null
  let pending: { readonly generation: number; readonly promise: Promise<readonly BoardListItem[]> } | null = null

  const load = (): Promise<readonly BoardListItem[]> => {
    if (cached !== null) return Promise.resolve(cached)
    if (pending !== null && pending.generation === generation) return pending.promise

    const requestGeneration = generation
    const controller = new AbortController()
    generationController = controller
    const promise = loadBoardList(runtime, { ...options, signal: controller.signal }).then(
      (items) => {
        if (requestGeneration !== generation) throw generationAbortError()
        cached = items
        pending = null
        generationController = null
        return items
      },
      (error: unknown) => {
        if (requestGeneration === generation) {
          pending = null
          generationController = null
        }
        throw error
      },
    )
    pending = { generation: requestGeneration, promise }
    return promise
  }

  const invalidate = (): void => {
    generationController?.abort()
    generationController = null
    generation += 1
    cached = null
    pending = null
  }

  return {
    load,
    reload() {
      invalidate()
      return load()
    },
    invalidate,
  }
}
