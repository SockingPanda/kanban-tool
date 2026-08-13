import type { ExplorerEvent } from "../../lib/api/events-read-model"
import type { Locale } from "../../lib/preferences"

const eventTimestampFormatters = new Map<Locale, Intl.DateTimeFormat>()
const eventTimestampOptions = { dateStyle: "medium", timeStyle: "short" } as const

export function eventTimestampFormatter(locale: Locale): Intl.DateTimeFormat {
  const cached = eventTimestampFormatters.get(locale)
  if (cached) return cached
  const language = locale === "en" ? "en-US" : "zh-CN"
  const formatter = new Intl.DateTimeFormat(language, eventTimestampOptions)
  eventTimestampFormatters.set(locale, formatter)
  return formatter
}

export function eventTimestamp(value: number, locale: Locale): { readonly display: string; readonly iso: string } {
  const milliseconds = Math.abs(value) < 1_000_000_000_000 ? value * 1_000 : value
  const date = new Date(milliseconds)
  if (Number.isNaN(date.getTime())) return { display: String(value), iso: String(value) }
  let display: string
  try {
    display = eventTimestampFormatter(locale).format(date)
  } catch {
    display = date.toISOString()
  }
  return { display, iso: date.toISOString() }
}

function stableJsonValue(value: unknown, seen: WeakSet<object>): unknown {
  if (value === null || typeof value !== "object") {
    return typeof value === "bigint" ? String(value) : value
  }
  if (seen.has(value)) return "[circular]"
  seen.add(value)
  try {
    if (Array.isArray(value)) return value.map((item) => stableJsonValue(item, seen))
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, stableJsonValue(value[key as keyof typeof value], seen)]),
    )
  } finally {
    seen.delete(value)
  }
}

export function eventPayloadJson(value: unknown): string {
  try {
    return JSON.stringify(stableJsonValue(value, new WeakSet<object>()), null, 2) ?? "null"
  } catch {
    return JSON.stringify("[unserializable payload]")
  }
}

export type EventRowIdentityProps = {
  readonly event: ExplorerEvent
  readonly locale: Locale
  readonly onSelectTask?: (taskId: string) => void
}

export function areEventRowPropsEqual(previous: EventRowIdentityProps, next: EventRowIdentityProps): boolean {
  return previous.event === next.event && previous.locale === next.locale && previous.onSelectTask === next.onSelectTask
}

export const __test = {
  areEventRowPropsEqual,
  eventTimestamp,
  eventTimestampFormatter,
  eventPayloadJson,
  resetEventTimestampFormatters: () => eventTimestampFormatters.clear(),
}
