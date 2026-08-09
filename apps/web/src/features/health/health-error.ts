import type { MessageKey } from "../../lib/i18n"
import { HealthReadError } from "../../lib/api/health-read-model"

type Translate = (key: MessageKey) => string

export type HealthErrorCopy = {
  readonly title: string
  readonly detail: string
  readonly nextStep: string
}

function statusSuffix(status: number | null): string {
  return status === null ? "" : ` (HTTP ${status})`
}

/** Present a health failure without rendering server-controlled messages. */
export function presentHealthError(error: unknown, t: Translate): HealthErrorCopy {
  if (!(error instanceof HealthReadError)) {
    return {
      title: t("healthUnavailable"),
      detail: t("healthUnexpectedError"),
      nextStep: t("healthRetryNextStep"),
    }
  }

  if (error.kind === "offline") {
    return {
      title: t("healthUnavailable"),
      detail: t("healthOfflineError"),
      nextStep: t("healthRetryNextStep"),
    }
  }

  if (error.kind === "http" && (error.code === "server_unavailable" || error.status === 503)) {
    return {
      title: t("healthUnavailable"),
      detail: `${t("healthServerUnavailable")}${statusSuffix(error.status)}`,
      nextStep: t("healthRetryNextStep"),
    }
  }

  if (error.kind === "http") {
    return {
      title: t("healthUnavailable"),
      detail: `${t("healthRequestFailed")}${statusSuffix(error.status)}`,
      nextStep: t("healthRetryNextStep"),
    }
  }

  if (
    error.kind === "invalid_json"
    || error.kind === "invalid_contract"
    || error.kind === "invalid_headers"
    || error.kind === "invalid_content_type"
    || error.kind === "invalid_bytes"
    || error.kind === "response_too_large"
  ) {
    return {
      title: t("healthUnavailable"),
      detail: t("healthInvalidResponse"),
      nextStep: t("healthRetryNextStep"),
    }
  }

  return {
    title: t("healthUnavailable"),
    detail: t("healthRequestFailed"),
    nextStep: t("healthRetryNextStep"),
  }
}
