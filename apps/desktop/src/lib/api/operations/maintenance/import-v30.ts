import { ApiTransport } from "../../transport"
import { expectExactKeys } from "../../parsers"
import { parseLegacyImportReport } from "./parsers"
import type { RequestOptions } from "../../types"

export async function importLegacySqliteV30(
  api: ApiTransport,
  path: string,
  canonicalAttachmentRoot: string | null = null,
  options: RequestOptions = {},
) {
  const envelope = await api.requestEnvelope<unknown>("/api/v1/maintenance/import-v30", {
    method: "POST",
    body: { path, canonical_attachment_root: canonicalAttachmentRoot },
    signal: options.signal,
  })
  expectExactKeys(envelope as unknown as Record<string, unknown>, ["data"], "legacy import response")
  return parseLegacyImportReport(envelope.data)
}
