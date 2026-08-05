import { ApiTransport } from "./transport"
import { expectArray, expectBoolean, expectExactKeys, expectRecord, expectSafeInteger, expectString } from "./parsers"
import { parseLabelOntologyActionEnvelope, parseLabelOntologyDetailEnvelope, parseLabelOntologyReviewGroup, parseLabelOntologySignal } from "./legacy/parsers"
import type { LabelAtomExplainRecord, LabelOntologyActionCreateInput, LabelOntologyReviewOptions, LabelOntologySignalListOptions, RequestOptions } from "./types"

export async function listLabelOntologySignals(api: ApiTransport, options: LabelOntologySignalListOptions = {}) {
    const params = new URLSearchParams({
      include_all: String(options.includeAll ?? false),
      limit: String(options.limit ?? 100),
    })
    for (const status of options.statuses ?? []) params.append("status", status)
    for (const kind of options.kinds ?? []) params.append("kind", kind)
    if (options.task?.trim()) params.set("task", options.task.trim())
    if (options.label?.trim()) params.set("label", options.label.trim())
    if (options.proposedLabel?.trim()) params.set("proposed_label", options.proposedLabel.trim())
    const response = expectRecord(
      await api.requestRaw(`/api/v1/boards/${api.board}/label-ontology/signals?${params.toString()}`, {
        signal: options.signal,
      }),
      "label ontology signals response",
    )
    expectExactKeys(response, ["data", "meta"], "label ontology signals response")
    const meta = expectRecord(response.meta, "label ontology signals response meta")
    expectExactKeys(meta, ["limit"], "label ontology signals response meta")
    expectSafeInteger(meta.limit, "label ontology signals response meta.limit", true)
    return expectArray<unknown>(response.data, "label ontology signals response data").map((entry, index) =>
      parseLabelOntologySignal(entry, `label ontology signals response data[${index}]`),
    )
  }


export async function reviewLabelOntology(api: ApiTransport, options: LabelOntologyReviewOptions = {}) {
    const params = new URLSearchParams({
      group_by: options.groupBy ?? "label",
      include_all: String(options.includeAll ?? false),
      limit: String(options.limit ?? 100),
    })
    const response = expectRecord<Record<string, unknown>>(await api.requestRaw(
      `/api/v1/boards/${api.board}/label-ontology/review?${params.toString()}`,
      { signal: options.signal },
    ), "label ontology review response")
    expectExactKeys(response, ["data", "meta"], "label ontology review response")
    const meta = expectRecord<Record<string, unknown>>(response.meta, "label ontology review response meta")
    expectExactKeys(meta, ["group_by", "include_all", "limit"], "label ontology review response meta")
    expectString(meta.group_by, "label ontology review response meta.group_by")
    expectBoolean(meta.include_all, "label ontology review response meta.include_all")
    expectSafeInteger(meta.limit, "label ontology review response meta.limit", true)
    return expectArray<unknown>(response.data, "label ontology review response data").map((entry, index) =>
      parseLabelOntologyReviewGroup(entry, `label ontology review response data[${index}]`),
    )
  }


export async function getLabelOntologySignal(api: ApiTransport, signalId: string, options: RequestOptions = {}) {
    return parseLabelOntologyDetailEnvelope(await api.requestRaw(
      `/api/v1/label-ontology/signals/${encodeURIComponent(signalId)}`, options,
    )).data
  }


export async function createLabelOntologyAction(api: ApiTransport, input: LabelOntologyActionCreateInput, options: RequestOptions = {}) {
    return parseLabelOntologyActionEnvelope(await api.requestRaw(`/api/v1/boards/${api.board}/label-ontology/actions`, {
      method: "POST",
      body: {
        actor: { name: api.actor, type: "user", agent_type: null },
        action_type: input.actionType,
        signal_ids: input.signalIds,
        reason: input.reason,
        superseded_by_signal_id: input.supersededBySignalId ?? null,
      },
      signal: options.signal,
    })).data
  }


export async function explainLabelAtom(api: ApiTransport, atomRef: string, options: RequestOptions = {}) {
    return api.request<LabelAtomExplainRecord>(
      `/api/v1/boards/${api.board}/labels/atoms/${encodeURIComponent(atomRef)}/explain`,
      options,
    )
  }
