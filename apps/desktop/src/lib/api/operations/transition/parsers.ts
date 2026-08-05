import { expectRecord, expectExactKeys, expectString, expectNullableInteger } from "../../parsers"
import type { ClaimResponse, Task } from "../../types"
import { parseApiTask } from "../task/parsers"
import { parseApiRun } from "../runs/parsers"

export function parseClaimEnvelope(value: unknown): ClaimResponse {
  const envelope = expectRecord<Record<string, unknown>>(value, "claim response")
  expectExactKeys(envelope, ["data"], "claim response")
  const data = expectRecord<Record<string, unknown>>(envelope.data, "claim response.data")
  expectExactKeys(data, ["task", "run", "claim_token", "claim_expires_at"], "claim response.data")
  return {
    task: parseApiTask(data.task, "claim response.data.task"),
    run: parseApiRun(data.run, "claim response.data.run"),
    claim_token: expectString(data.claim_token, "claim response.data.claim_token"),
    claim_expires_at: expectNullableInteger(data.claim_expires_at, "claim response.data.claim_expires_at"),
  }
}


export function parseTransitionTaskEnvelope(value: unknown): Task {
  const envelope = expectRecord<Record<string, unknown>>(value, "task transition response")
  expectExactKeys(envelope, ["data"], "task transition response")
  return parseApiTask(envelope.data, "task transition response.data")
}
