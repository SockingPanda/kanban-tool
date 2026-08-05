import { ApiTransport } from "../../transport"
import { parseCreateBoardEnvelope } from "./parsers"
import type { CreateBoardInput, RequestOptions } from "../../types"

export async function createBoard(api: ApiTransport, input: CreateBoardInput, options: RequestOptions = {}) {
    return parseCreateBoardEnvelope(await api.requestRaw("/api/v1/boards", {
      method: "POST",
      body: {
        slug: input.slug,
        name: input.name,
        description: input.description ?? null,
        actor: api.actor,
      },
      actorHeader: true,
      signal: options.signal,
    })).data
  }
