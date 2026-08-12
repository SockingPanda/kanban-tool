/* eslint-disable react-refresh/only-export-components */

import {
  forwardRef,
  useEffect,
  useId,
  useRef,
  useState,
  type AriaAttributes,
  type ChangeEvent,
  type FieldsetHTMLAttributes,
  type Ref,
} from "react"

export type ISODateTimeString = string & {
  readonly __brand: "ISODateTimeString"
}

export type DateTimeInputStatusType = "warning" | "error" | "success"

export interface DateTimeInputStatus {
  readonly type: DateTimeInputStatusType
  readonly message?: string
}

export type DateTimeInputSize = "sm" | "md" | "lg"
export type DateTimeInputTimeIncrement = 1 | 5 | 10 | 15 | 30

export interface ISODateTimeParts {
  readonly date: string
  readonly time?: string
}

export type DateTimeInputAriaDataProps = AriaAttributes & {
  readonly [key: `data-${string}`]: string | number | boolean | undefined
}

const DATE_PATTERN = /^(\d{4})-(\d{2})-(\d{2})$/
const TIME_PATTERN = /^(\d{2}):(\d{2})(?::(\d{2}))?$/
const DATE_TIME_PATTERN = /^(\d{4}-\d{2}-\d{2})T(\d{2}:\d{2}(?::\d{2})?)$/

const FIELD_CLASSES = "grid min-w-0 gap-2 border-0 p-0 text-primary"
const LABEL_CLASSES = "flex items-baseline gap-1 text-sm font-normal leading-normal"
const DESCRIPTION_CLASSES = "m-0 text-sm leading-normal text-secondary"
const ROW_CLASSES = "grid min-w-0 grid-cols-1 gap-2 sm:grid-cols-2"
const CONTROL_CLASSES = "min-w-0 rounded-md border border-border-strong bg-surface px-3 py-2 text-sm leading-normal text-primary outline-none placeholder:text-secondary focus-visible:border-accent focus-visible:outline focus-visible:outline-accent disabled:cursor-not-allowed disabled:opacity-50"
const CLEAR_CLASSES = "justify-self-start rounded-md border border-border-strong bg-surface px-3 py-2 text-sm leading-normal text-primary outline-none focus-visible:border-accent focus-visible:outline focus-visible:outline-accent disabled:cursor-not-allowed disabled:opacity-50"
const STATUS_MESSAGE_CLASSES = "m-0 rounded-md border px-2 py-2 text-sm leading-normal"
const VISUALLY_HIDDEN = "sr-only"

const STATUS_BORDER_CLASSES: Readonly<Record<DateTimeInputStatusType, string>> = {
  warning: "border-warning",
  error: "border-error",
  success: "border-success",
}

const STATUS_MESSAGE_VARIANT_CLASSES: Readonly<Record<DateTimeInputStatusType, string>> = {
  warning: "border-warning bg-warning-muted text-warning",
  error: "border-error bg-error-muted text-error",
  success: "border-success bg-success-muted text-success",
}

const ROOT_FORWARD_KEYS = new Set([
  "dir",
  "draggable",
  "hidden",
  "inert",
  "lang",
  "role",
  "tabIndex",
  "title",
])

type RootForwardProps = Record<string, unknown>

function joinClasses(
  ...classes: ReadonlyArray<string | false | null | undefined>
): string | undefined {
  const merged = classes.filter(Boolean).join(" ")
  return merged || undefined
}

function joinIds(
  ...ids: ReadonlyArray<string | undefined>
): string | undefined {
  const seen = new Set<string>()
  const result: string[] = []

  for (const id of ids) {
    for (const token of id?.split(/\s+/) ?? []) {
      if (token && !seen.has(token)) {
        seen.add(token)
        result.push(token)
      }
    }
  }

  return result.length > 0 ? result.join(" ") : undefined
}

function assignRef<T>(ref: Ref<T> | undefined, value: T | null): void {
  if (typeof ref === "function") {
    ref(value)
  } else if (ref) {
    ref.current = value
  }
}

function pickRootProps(props: RootForwardProps): RootForwardProps {
  return Object.fromEntries(
    Object.entries(props).filter(([key]) =>
      key.startsWith("aria-") || key.startsWith("data-") || ROOT_FORWARD_KEYS.has(key),
    ),
  )
}

function daysInMonth(year: number, month: number): number {
  if (month === 2) {
    const leap = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0)
    return leap ? 29 : 28
  }
  return [4, 6, 9, 11].includes(month) ? 30 : 31
}

export function isISODate(value: string): boolean {
  const match = DATE_PATTERN.exec(value)
  if (!match) return false

  const year = Number(match[1])
  const month = Number(match[2])
  const day = Number(match[3])
  return year >= 1 && month >= 1 && month <= 12 && day >= 1 && day <= daysInMonth(year, month)
}

export function isISOTime(value: string, hasSeconds = true): boolean {
  const match = TIME_PATTERN.exec(value)
  if (!match) return false
  if (!hasSeconds && match[3] !== undefined) return false

  const hour = Number(match[1])
  const minute = Number(match[2])
  const second = Number(match[3] ?? 0)
  return hour >= 0 && hour <= 23 && minute >= 0 && minute <= 59 && second >= 0 && second <= 59
}

function normalizeISOTime(value: string, hasSeconds = true): string | undefined {
  const match = TIME_PATTERN.exec(value)
  if (!match || !isISOTime(value, true)) return undefined
  if (!hasSeconds && match[3] !== undefined) return `${match[1]}:${match[2]}`
  return value
}

export function parseISODateTime(
  value: string | null | undefined,
  hasSeconds = true,
): ISODateTimeParts | undefined {
  const candidate = value?.trim()
  if (!candidate) return undefined

  if (isISODate(candidate)) return { date: candidate }

  const match = DATE_TIME_PATTERN.exec(candidate)
  const time = match ? normalizeISOTime(match[2], hasSeconds) : undefined
  if (!match || !isISODate(match[1]) || !time) return undefined
  return { date: match[1], time }
}

export function parseDateTimeLocal(
  date: string | null | undefined,
  time?: string | null,
  hasSeconds = true,
): ISODateTimeString | undefined {
  if (time === undefined && date?.includes("T")) {
    const parsed = parseISODateTime(date, hasSeconds)
    return parsed?.time ? parseDateTimeLocal(parsed.date, parsed.time, hasSeconds) : undefined
  }

  const dateValue = date?.trim() ?? ""
  const timeValue = normalizeISOTime(time?.trim() ?? "", hasSeconds)
  if (!isISODate(dateValue) || !timeValue) return undefined
  return `${dateValue}T${timeValue}` as ISODateTimeString
}

export function splitDateTimeValue(
  value: string | null | undefined,
  hasSeconds = true,
): ISODateTimeParts | undefined {
  return parseISODateTime(value, hasSeconds)
}

export function combineDateTimeValue(
  date: string | null | undefined,
  time: string | null | undefined,
  hasSeconds = true,
): ISODateTimeString | undefined {
  return parseDateTimeLocal(date, time, hasSeconds)
}

export const parseLocalDateTime = parseDateTimeLocal
export const formatISODateTime = combineDateTimeValue

export interface DateTimeChangeResolution {
  readonly accepted: boolean
  readonly date: string
  readonly time: string
  readonly value?: ISODateTimeString
}

function compareISODate(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0
}

function timeToSeconds(value: string): number {
  const [hour, minute, second = "0"] = value.split(":")
  return Number(hour) * 3600 + Number(minute) * 60 + Number(second)
}

export function isDateWithinBounds(
  date: string | null | undefined,
  min?: ISODateTimeParts,
  max?: ISODateTimeParts,
): boolean {
  const dateValue = date?.trim() ?? ""
  if (!isISODate(dateValue)) return false
  if (min?.date && compareISODate(dateValue, min.date) < 0) return false
  if (max?.date && compareISODate(dateValue, max.date) > 0) return false
  return true
}

export function isDateTimeWithinBounds(
  date: string | null | undefined,
  time: string | null | undefined,
  min?: ISODateTimeParts,
  max?: ISODateTimeParts,
  hasSeconds = true,
): boolean {
  const dateValue = date?.trim() ?? ""
  const timeValue = normalizeISOTime(time?.trim() ?? "", hasSeconds)
  if (!timeValue || !isDateWithinBounds(dateValue, min, max)) return false

  if (min?.date === dateValue && min.time) {
    const minTime = normalizeISOTime(min.time, hasSeconds)
    if (minTime && timeToSeconds(timeValue) < timeToSeconds(minTime)) return false
  }
  if (max?.date === dateValue && max.time) {
    const maxTime = normalizeISOTime(max.time, hasSeconds)
    if (maxTime && timeToSeconds(timeValue) > timeToSeconds(maxTime)) return false
  }
  return true
}

export function resolveDateTimeChange(
  date: string | null | undefined,
  time: string | null | undefined,
  min?: ISODateTimeParts,
  max?: ISODateTimeParts,
  hasSeconds = true,
): DateTimeChangeResolution {
  const dateValue = date?.trim() ?? ""
  const rawTime = time?.trim() ?? ""
  const timeValue = normalizeISOTime(rawTime, hasSeconds) ?? ""

  if (dateValue && (!isISODate(dateValue) || !isDateWithinBounds(dateValue, min, max))) {
    return { accepted: false, date: dateValue, time: timeValue }
  }
  if (rawTime && !timeValue) {
    return { accepted: false, date: dateValue, time: timeValue }
  }
  if (!dateValue || !timeValue) {
    return { accepted: true, date: dateValue, time: timeValue }
  }

  const value = parseDateTimeLocal(dateValue, timeValue, hasSeconds)
  if (!value || !isDateTimeWithinBounds(dateValue, timeValue, min, max, hasSeconds)) {
    return { accepted: false, date: dateValue, time: timeValue }
  }
  return { accepted: true, date: dateValue, time: timeValue, value }
}

function statusTypeOf(
  status: DateTimeInputStatus | DateTimeInputStatusType | undefined,
): DateTimeInputStatusType | undefined {
  return typeof status === "string" ? status : status?.type
}

function statusMessageOf(
  status: DateTimeInputStatus | DateTimeInputStatusType | undefined,
): string | undefined {
  return typeof status === "object" ? status.message : undefined
}

type NativeFieldsetProps = Omit<
  FieldsetHTMLAttributes<HTMLFieldSetElement>,
  | "children"
  | "className"
  | "id"
  | "name"
  | "onChange"
  | "style"
  | "value"
  | "disabled"
  | "required"
>

export interface DateTimeInputProps extends NativeFieldsetProps, DateTimeInputAriaDataProps {
  readonly ref?: Ref<HTMLInputElement>
  readonly id?: string
  readonly className?: string
  readonly label: string
  readonly description?: string
  readonly status?: DateTimeInputStatus | DateTimeInputStatusType
  readonly htmlName?: string
  readonly value?: ISODateTimeString | string
  readonly onChange?: (value: ISODateTimeString | undefined) => void
  readonly clear?: boolean
  readonly hasClear?: boolean
  readonly isLabelHidden?: boolean
  readonly isOptional?: boolean
  readonly isRequired?: boolean
  readonly isDisabled?: boolean
  readonly isLoading?: boolean
  readonly disabledMessage?: string
  readonly placeholder?: string
  readonly timePlaceholder?: string
  readonly dateLabel: string
  readonly timeLabel: string
  readonly clearLabel: string
  readonly clearText?: string
  readonly optionalText?: string
  readonly hasSeconds?: boolean
  readonly timeIncrement?: DateTimeInputTimeIncrement
  readonly size?: DateTimeInputSize
  readonly statusVariant?: "attached" | "detached" | "tooltip"
  readonly min?: ISODateTimeString | string
  readonly max?: ISODateTimeString | string
  readonly autoComplete?: string
  readonly translate?: "yes" | "no"
  readonly onDateChange?: never
}

function DateTimeInputImpl(
  {
    id: providedId,
    className,
    label,
    description,
    status,
    htmlName,
    value,
    onChange,
    clear = false,
    hasClear = false,
    isLabelHidden = false,
    isOptional = false,
    isRequired = false,
    isDisabled = false,
    isLoading = false,
    disabledMessage,
    placeholder,
    timePlaceholder,
    dateLabel,
    timeLabel,
    clearLabel,
    clearText = "×",
    optionalText,
    hasSeconds = false,
    timeIncrement,
    size = "md",
    statusVariant = "attached",
    min,
    max,
    autoComplete = "off",
    translate,
    ["aria-describedby"]: callerDescribedBy,
    ["aria-invalid"]: callerInvalid,
    ["aria-required"]: callerRequired,
    ["aria-disabled"]: callerDisabled,
    ["aria-busy"]: callerBusy,
    ["aria-label"]: callerLabel,
    ["aria-labelledby"]: callerLabelledBy,
    onFocus,
    onBlur,
    onClick,
    onKeyDown,
    onKeyUp,
    onMouseDown,
    ...rest
  }: DateTimeInputProps,
  forwardedRef: Ref<HTMLInputElement>,
) {
  const generatedId = useId().replaceAll(":", "")
  const controlId = providedId ?? `date-time-${generatedId}`
  const dateId = controlId
  const timeId = `${controlId}-time`
  const descriptionId = description ? `${controlId}-description` : undefined
  const statusId = statusMessageOf(status) ? `${controlId}-status` : undefined
  const legendId = `${controlId}-legend`
  const dateInputRef = useRef<HTMLInputElement | null>(null)
  const parsedValue = parseISODateTime(value, hasSeconds)
  const [dateValue, setDateValue] = useState(parsedValue?.date ?? "")
  const [timeValue, setTimeValue] = useState(parsedValue?.time ?? "")
  const dateBounds = parseISODateTime(min, hasSeconds)
  const maxBounds = parseISODateTime(max, hasSeconds)
  const statusType = statusTypeOf(status)
  const statusMessage = statusMessageOf(status)
  const interactiveDisabled = isDisabled || isLoading
  const disabledMessageId = interactiveDisabled && disabledMessage ? `${controlId}-disabled` : undefined
  const describedBy = joinIds(callerDescribedBy, descriptionId, statusId, disabledMessageId)
  const rootProps = pickRootProps(rest)

  useEffect(() => {
    const next = parseISODateTime(value, hasSeconds)
    setDateValue(next?.date ?? "")
    setTimeValue(next?.time ?? "")
  }, [hasSeconds, value])

  const setDateRef = (node: HTMLInputElement | null) => {
    dateInputRef.current = node
    assignRef(forwardedRef, node)
  }

  const fireChange = (nextDate: string, nextTime: string): DateTimeChangeResolution => {
    const resolution = resolveDateTimeChange(nextDate, nextTime, dateBounds, maxBounds, hasSeconds)
    if (resolution.accepted) onChange?.(resolution.value)
    return resolution
  }

  const handleDateChange = (event: ChangeEvent<HTMLInputElement>) => {
    if (interactiveDisabled) return
    const nextDate = event.currentTarget.value.trim()
    const resolution = fireChange(nextDate, timeValue)
    if (resolution.accepted) {
      setDateValue(resolution.date)
    } else {
      event.currentTarget.value = dateValue
    }
  }

  const handleTimeChange = (event: ChangeEvent<HTMLInputElement>) => {
    if (interactiveDisabled) return
    const rawTime = event.currentTarget.value.trim()
    const resolution = fireChange(dateValue, rawTime)
    if (resolution.accepted) {
      setTimeValue(resolution.time)
    } else {
      event.currentTarget.value = timeValue
    }
  }

  const handleClear = () => {
    if (interactiveDisabled) return
    setDateValue("")
    setTimeValue("")
    onChange?.(undefined)
    dateInputRef.current?.focus()
  }

  const nativeStatusBorder = statusType ? STATUS_BORDER_CLASSES[statusType] : undefined
  const inputClassName = joinClasses(CONTROL_CLASSES, nativeStatusBorder)
  const rootAriaLabelledBy = joinIds(callerLabelledBy, legendId)
  const ariaInvalid = statusType === "error" ? true : callerInvalid
  const ariaRequired = isRequired && !isOptional ? true : callerRequired
  const ariaDisabled = interactiveDisabled ? true : callerDisabled
  const ariaBusy = isLoading ? true : callerBusy
  const clearEnabled = (clear || hasClear) && Boolean(dateValue || timeValue)
  const timeMin = dateValue && dateBounds?.date === dateValue ? dateBounds.time : undefined
  const timeMax = dateValue && maxBounds?.date === dateValue ? maxBounds.time : undefined

  return (
    <fieldset
      {...rootProps}
      aria-busy={ariaBusy}
      aria-describedby={describedBy}
      aria-disabled={ariaDisabled}
      aria-invalid={ariaInvalid}
      aria-label={callerLabel}
      aria-labelledby={rootAriaLabelledBy}
      aria-required={ariaRequired}
      className={joinClasses(FIELD_CLASSES, className)}
      data-astryx-component="date-time-input"
      data-disabled={isDisabled || undefined}
      data-size={size}
      data-status={statusType}
      data-time-increment={timeIncrement}
      onBlur={onBlur}
      onClick={onClick}
      onFocus={onFocus}
      onKeyDown={onKeyDown}
      onKeyUp={onKeyUp}
      onMouseDown={onMouseDown}
    >
      <legend id={legendId} className={joinClasses(LABEL_CLASSES, isLabelHidden && VISUALLY_HIDDEN)}>
        {label}
        {isRequired && !isOptional ? <small aria-hidden="true"> *</small> : null}
        {isOptional && optionalText ? <small aria-hidden="true"> {optionalText}</small> : null}
      </legend>
      {description ? <p id={descriptionId} className={DESCRIPTION_CLASSES}>{description}</p> : null}
      <section className={ROW_CLASSES} aria-label={label} data-astryx-segment="row">
        <label className={VISUALLY_HIDDEN} htmlFor={dateId}>{dateLabel}</label>
        <input
          ref={setDateRef}
          aria-busy={ariaBusy}
          aria-describedby={describedBy}
          aria-disabled={ariaDisabled}
          aria-invalid={ariaInvalid}
          aria-required={ariaRequired}
          autoComplete={autoComplete}
          className={inputClassName}
          data-astryx-segment="date"
          disabled={interactiveDisabled}
          id={dateId}
          max={maxBounds?.date}
          min={dateBounds?.date}
          name={htmlName}
          onChange={handleDateChange}
          placeholder={placeholder}
          required={isRequired && !isOptional}
          translate={translate}
          type="date"
          value={dateValue}
        />
        <label className={VISUALLY_HIDDEN} htmlFor={timeId}>{timeLabel}</label>
        <input
          aria-busy={ariaBusy}
          aria-describedby={describedBy}
          aria-disabled={ariaDisabled}
          aria-invalid={ariaInvalid}
          aria-required={ariaRequired}
          autoComplete={autoComplete}
          className={inputClassName}
          data-astryx-segment="time"
          disabled={interactiveDisabled}
          id={timeId}
          max={timeMax}
          min={timeMin}
          name={htmlName ? `${htmlName}-time` : undefined}
          onChange={handleTimeChange}
          placeholder={timePlaceholder}
          required={isRequired && !isOptional}
          step={hasSeconds ? 1 : (timeIncrement ?? 1) * 60}
          translate={translate}
          type="time"
          value={timeValue}
        />
      </section>
      {clearEnabled ? (
        <button
          aria-label={clearLabel}
          className={CLEAR_CLASSES}
          disabled={interactiveDisabled}
          onClick={handleClear}
          type="button"
        >
          {clearText}
        </button>
      ) : null}
      {statusMessage ? (
        <p
          aria-live={statusType === "error" ? "assertive" : "polite"}
          className={joinClasses(STATUS_MESSAGE_CLASSES, statusType ? STATUS_MESSAGE_VARIANT_CLASSES[statusType] : undefined)}
          data-status={statusType}
          data-variant={statusVariant}
          id={statusId}
          role={statusType === "error" ? "alert" : "status"}
        >
          {statusMessage}
        </p>
      ) : null}
      {interactiveDisabled && disabledMessage ? <p id={disabledMessageId} className={DESCRIPTION_CLASSES}>{disabledMessage}</p> : null}
    </fieldset>
  )
}

export const DateTimeInput = forwardRef<HTMLInputElement, DateTimeInputProps>(DateTimeInputImpl)
DateTimeInput.displayName = "DateTimeInput"

export default DateTimeInput
