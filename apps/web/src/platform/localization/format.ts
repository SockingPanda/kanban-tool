import { localeTag, type Locale } from "./locale";

function finite(value: number): number {
  if (!Number.isFinite(value)) throw new RangeError("Expected a finite number");
  return value;
}

/** 日历日期不随查看者时区偏移。 */
export function parseCalendarDate(value: string): Date {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!match) throw new RangeError("Expected YYYY-MM-DD");
  const [, yy, mm, dd] = match;
  const y = Number(yy), m = Number(mm), d = Number(dd);
  if (y < 1 || m < 1 || m > 12 || d < 1 || d > 31) throw new RangeError("Invalid calendar date");
  const date = new Date(0);
  date.setUTCFullYear(y, m - 1, d);
  date.setUTCHours(0, 0, 0, 0);
  if (date.getUTCFullYear() !== y || date.getUTCMonth() !== m - 1 || date.getUTCDate() !== d) throw new RangeError("Invalid calendar date");
  return date;
}

/** 时刻必须带 UTC 偏移，或使用 epoch / Date。 */
export function parseInstant(value: string | number | Date): Date {
  if (typeof value === "string") {
    if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,3})?(?:Z|[+-]\d{2}:\d{2})$/.test(value)) throw new RangeError("Timestamp requires an explicit UTC offset");
    parseCalendarDate(value.slice(0, 10));
    const clock = value.slice(11, 19).split(":").map(Number);
    if (clock[0] > 23 || clock[1] > 59 || clock[2] > 59) throw new RangeError("Invalid timestamp time");
    if (!value.endsWith("Z")) {
      const [h, m] = value.slice(-5).split(":").map(Number);
      if (h > 23 || m > 59) throw new RangeError("Invalid UTC offset");
    }
  }
  const date = value instanceof Date ? new Date(value.getTime()) : new Date(value);
  if (!Number.isFinite(date.getTime())) throw new RangeError("Invalid timestamp");
  return date;
}

/** 按语言和实际选项复用 Intl 实例；选项变化仍使用独立实例，缓存保持有界。 */
function cachedFormatter<Options extends object, Formatter>(Constructor: new (locale: string, options: Options) => Formatter) {
  const cache = new Map<string, Formatter>();
  return (locale: string, options: Options): Formatter => {
    const entries = Object.entries(options).filter(([, value]) => value !== undefined).sort(([a], [b]) => a.localeCompare(b, "en"));
    const key = JSON.stringify([locale, entries]);
    let formatter = cache.get(key);
    if (formatter === undefined) {
      formatter = new Constructor(locale, options);
      if (cache.size >= 64) cache.clear();
      cache.set(key, formatter);
    }
    return formatter;
  };
}

const numberFormatter = cachedFormatter<Intl.NumberFormatOptions, Intl.NumberFormat>(Intl.NumberFormat);
const dateFormatter = cachedFormatter<Intl.DateTimeFormatOptions, Intl.DateTimeFormat>(Intl.DateTimeFormat);
const relativeFormatter = cachedFormatter<Intl.RelativeTimeFormatOptions, Intl.RelativeTimeFormat>(Intl.RelativeTimeFormat);

export function createFormatters(locale: Locale, timeZone?: string) {
  const tag = localeTag(locale);
  return {
    number(value: number, options: Intl.NumberFormatOptions = {}) {
      return numberFormatter(tag, options).format(finite(value));
    },
    percent(ratio: number, maximumFractionDigits = 0) {
      return numberFormatter(tag, { style: "percent", maximumFractionDigits }).format(finite(ratio));
    },
    calendarDate(value: string, options: Omit<Intl.DateTimeFormatOptions, "timeZone"> = { dateStyle: "medium" }) {
      return dateFormatter(tag, { ...options, timeZone: "UTC" }).format(parseCalendarDate(value));
    },
    instant(value: string | number | Date, options: Intl.DateTimeFormatOptions = { dateStyle: "medium", timeStyle: "short" }) {
      return dateFormatter(tag, { timeZone, ...options }).format(parseInstant(value));
    },
    relative(value: number, unit: Intl.RelativeTimeFormatUnit, options: Intl.RelativeTimeFormatOptions = { numeric: "auto" }) {
      return relativeFormatter(tag, options).format(finite(value), unit);
    },
  };
}
