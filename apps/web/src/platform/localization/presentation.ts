import type { MessageKey, Translator } from "./contracts.generated";

const statusKeys = {
  triage: "tasks:status.triage", todo: "tasks:status.todo", scheduled: "tasks:status.scheduled",
  ready: "tasks:status.ready", running: "tasks:status.running", blocked: "tasks:status.blocked",
  review: "tasks:status.review", done: "tasks:status.done", archived: "tasks:status.archived",
} as const satisfies Record<string, MessageKey>;

/** Unknown/custom statuses remain user or server content. Never translate IDs in a request. */
export function taskStatusLabel(status: string, t: Translator): string {
  return Object.prototype.hasOwnProperty.call(statusKeys, status) ? t(statusKeys[status as keyof typeof statusKeys]) : status;
}

const errorKeys = {
  CONFLICT: "common:errorConflict", UNAVAILABLE: "common:errorUnavailable", INVALID_ARGUMENT: "common:errorInvalidInput",
} as const satisfies Record<string, MessageKey>;
/** Transport-neutral presentation helper. The transport adapter must map its real error codes explicitly. */
export function errorLabel(code: string, t: Translator): string {
  return Object.prototype.hasOwnProperty.call(errorKeys, code) ? t(errorKeys[code as keyof typeof errorKeys]) : t("common:errorUnknown");
}
