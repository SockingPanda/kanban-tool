import type { SignalsOntologyReadApi } from "../../lib/api/signals-ontology-read-model"

export type LifecycleAction = "confirm" | "reject" | "resolve_no_change"

export interface OntologyLifecycleIdentity {
  readonly api: SignalsOntologyReadApi | null
  readonly signalId: string | null
}

export interface OntologyLifecycleAttempt {
  readonly action: LifecycleAction
  readonly signalId: string
  readonly reason: string
  readonly identity: OntologyLifecycleIdentity
}

export type OntologyLifecycleExecutionResult =
  | { readonly kind: "success"; readonly attempt: OntologyLifecycleAttempt }
  | { readonly kind: "error"; readonly attempt: OntologyLifecycleAttempt; readonly error: unknown }
  | { readonly kind: "stale"; readonly attempt: OntologyLifecycleAttempt }

export function sameOntologyLifecycleIdentity(left: OntologyLifecycleIdentity, right: OntologyLifecycleIdentity): boolean {
  return left.api === right.api && left.signalId === right.signalId
}

export function createOntologyLifecycleAttempt(
  action: LifecycleAction,
  signalId: string,
  reason: string,
  identity: OntologyLifecycleIdentity,
): OntologyLifecycleAttempt {
  return Object.freeze({ action, signalId, reason, identity })
}

export async function executeOntologyLifecycleAttempt(
  attempt: OntologyLifecycleAttempt,
  execute: (action: LifecycleAction, signalId: string, reason: string) => void | Promise<void>,
  isCurrent: () => boolean,
): Promise<OntologyLifecycleExecutionResult> {
  try {
    await execute(attempt.action, attempt.signalId, attempt.reason)
  } catch (error) {
    return isCurrent() ? { kind: "error", attempt, error } : { kind: "stale", attempt }
  }
  return isCurrent() ? { kind: "success", attempt } : { kind: "stale", attempt }
}
