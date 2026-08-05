import { ApiError } from "./errors"
import type { ApiEnvelope, ErrorEnvelope, PageEnvelopeMeta, PageMeta, RequiredOffsetPageMeta, RequiredTotalPageMeta } from "./types"

export { ApiError } from "./errors"

export function parseJsonEnvelope<T, M>(text: string): ApiEnvelope<T, M> | ErrorEnvelope | null {
  if (!text) return null
  try { return JSON.parse(text) as ApiEnvelope<T, M> | ErrorEnvelope }
  catch { return null }
}

export function expectArray<T>(value: unknown, label: string): T[] {
  if (!Array.isArray(value)) throw new ApiError("invalid_response", `${label} must be an array`)
  return value as T[]
}

export function expectRecord<T extends Record<string, unknown>>(value: unknown, label: string): T {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new ApiError("invalid_response", `${label} must be an object`)
  return value as T
}

export function normalizePageMeta(meta: PageEnvelopeMeta | undefined, fallback: { limit: number; offset: number }): PageMeta {
  const limit = numericMeta(meta?.limit, fallback.limit)
  const offset = numericMeta(meta?.offset, fallback.offset)
  const total = typeof meta?.total === "number" && Number.isFinite(meta.total) ? meta.total : null
  return { limit, offset, total }
}

export function numericMeta(value: unknown, fallback: number) {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback
}

export function expectRequiredOffsetPageMeta(value: unknown, label: string): RequiredOffsetPageMeta {
  const meta = expectRecord<Record<string, unknown>>(value, label)
  return { limit: expectFiniteNumber(meta.limit, label + ".limit"), offset: expectFiniteNumber(meta.offset, label + ".offset") }
}

export function expectRequiredTotalPageMeta(value: unknown, label: string): RequiredTotalPageMeta {
  const meta = expectRequiredOffsetPageMeta(value, label)
  const record = expectRecord<Record<string, unknown>>(value, label)
  return { ...meta, total: expectFiniteNumber(record.total, label + ".total") }
}

export function expectFiniteNumber(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) throw new ApiError("invalid_response", label + " must be a finite number")
  return value
}

export function expectExactKeys(record: Record<string, unknown>, expected: readonly string[], label: string) {
  const actual = Object.keys(record)
  if (actual.length !== expected.length || actual.some((key) => !expected.includes(key))) throw new ApiError("invalid_response", `${label} must contain exactly: ${expected.join(", ")}`)
}

export function expectString(value: unknown, label: string): string {
  if (typeof value !== "string") throw new ApiError("invalid_response", `${label} must be a string`)
  return value
}

export function expectBoolean(value: unknown, label: string): boolean {
  if (typeof value !== "boolean") throw new ApiError("invalid_response", `${label} must be a boolean`)
  return value
}

export function expectSafeInteger(value: unknown, label: string, nonNegative = false): number {
  if (!Number.isSafeInteger(value) || (nonNegative && (value as number) < 0)) throw new ApiError("invalid_response", `${label} must be ${nonNegative ? "a non-negative " : "a "}safe integer`)
  return value as number
}

export function expectNullableString(value: unknown, label: string): string | null { return value === null ? null : expectString(value, label) }
export function expectNullableInteger(value: unknown, label: string): number | null { return value === null ? null : expectSafeInteger(value, label) }
