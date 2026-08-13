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
  resetEventTimestampFormatters: () => eventTimestampFormatters.clear(),
}
