/* eslint-disable react-refresh/only-export-components */
import type {
  AriaAttributes,
  ReactNode,
} from "react"

import type {CspSafeDomProps} from "../dom-props"

export type FieldStatusType = "warning" | "error" | "success"
/** The safe shell renders status text as an attached sibling message. */
export type FieldStatusVariant = "attached"

export interface FieldStatus {
  readonly type: FieldStatusType
  readonly message?: string
}

/** Native fields accept aria-* and data-* without opening a presentation API. */
export type AriaDataProps = AriaAttributes & {
  readonly [name: `data-${string}`]: string | number | boolean | undefined
}

export type CommonFieldProps<E extends HTMLElement> = Omit<CspSafeDomProps<E>, "id" | "className"> & {
  readonly id?: string
  readonly className?: string
  readonly autoComplete?: string
}

/** Literal utility classes use only published Astryx token variables. */
export const FIELD_CLASSES =
  "grid gap-2 text-primary"
export const LABEL_CLASSES =
  "flex items-baseline gap-1 text-sm font-normal leading-normal"
export const DESCRIPTION_CLASSES =
  "m-0 text-sm leading-normal text-secondary"
export const CONTROL_CLASSES =
  "min-w-0 rounded-md border border-border-strong bg-surface px-3 py-2 text-sm leading-normal text-primary outline-none placeholder:text-secondary focus-visible:border-accent focus-visible:outline focus-visible:outline-accent disabled:cursor-not-allowed disabled:opacity-50"
export const TEXTAREA_CLASSES =
  "block w-full min-w-0 resize-y rounded-md border border-border-strong bg-surface px-3 py-2 text-sm leading-normal text-primary outline-none placeholder:text-secondary focus-visible:border-accent focus-visible:outline focus-visible:outline-accent disabled:cursor-not-allowed disabled:opacity-50"
export const CHECKBOX_CLASSES =
  "h-5 w-5 accent-accent-bg focus-visible:outline focus-visible:outline-accent disabled:cursor-not-allowed disabled:opacity-50"
export const FILE_CLASSES =
  "block w-full cursor-pointer rounded-md border border-dashed border-border-strong bg-surface px-3 py-2 text-sm leading-normal text-primary file:mr-2 file:rounded-md file:border-0 file:bg-accent-bg file:px-2 file:py-1 file:text-on-accent focus-visible:border-accent focus-visible:outline focus-visible:outline-accent disabled:cursor-not-allowed disabled:opacity-50"
export const COUNTER_CLASSES =
  "m-0 text-end text-sm leading-normal text-secondary"
export const STATUS_MESSAGE_CLASSES =
  "m-0 rounded-md border px-2 py-2 text-sm leading-normal"
export const VISUALLY_HIDDEN = "sr-only"

export const STATUS_CLASSES: Record<FieldStatusType, string> = {
  warning:
    "border-warning bg-warning-muted text-warning",
  error:
    "border-error bg-error-muted text-error",
  success:
    "border-success bg-success-muted text-success",
}

export const STATUS_BORDER_CLASSES: Record<FieldStatusType, string> = {
  warning: "border-warning",
  error: "border-error",
  success: "border-success",
}

export function mergeClasses(
  ...classes: ReadonlyArray<string | false | null | undefined>
): string | undefined {
  const merged = classes.filter(Boolean).join(" ")
  return merged || undefined
}

/** Keep caller-provided and generated ids stable, ordered, and unique. */
export function mergeDescribedBy(
  ...values: ReadonlyArray<string | undefined>
): string | undefined {
  const seen = new Set<string>()
  const tokens: string[] = []

  for (const value of values) {
    for (const token of value?.split(/\s+/) ?? []) {
      if (token && !seen.has(token)) {
        seen.add(token)
        tokens.push(token)
      }
    }
  }

  return tokens.length > 0 ? tokens.join(" ") : undefined
}

export function statusControlClasses(status?: FieldStatus): string | undefined {
  return status ? STATUS_BORDER_CLASSES[status.type] : undefined
}

export interface FieldShellProps {
  readonly label: string
  readonly controlId: string
  readonly labelHidden?: boolean
  readonly description?: string
  readonly descriptionId?: string
  readonly required?: boolean
  readonly optional?: boolean
  readonly requiredText?: string
  readonly optionalText?: string
  readonly disabled?: boolean
  readonly status?: FieldStatus
  readonly statusVariant?: FieldStatusVariant
  readonly disabledMessage?: string
  readonly disabledMessageId?: string
  readonly children: ReactNode
  readonly className?: string
}

/** Semantic label/description/status shell with no runtime presentation hooks. */
export function FieldShell({
  label,
  controlId,
  labelHidden = false,
  description,
  descriptionId,
  required = false,
  optional = false,
  requiredText,
  optionalText,
  disabled = false,
  status,
  statusVariant = "attached",
  disabledMessage,
  disabledMessageId,
  children,
  className,
}: FieldShellProps) {
  const statusId = status?.message ? `${controlId}-status` : undefined

  return (
    <section
      className={mergeClasses(FIELD_CLASSES, className)}
      data-astryx-field="true"
      data-disabled={disabled || undefined}
    >
      <label
        className={mergeClasses(LABEL_CLASSES, labelHidden && VISUALLY_HIDDEN)}
        htmlFor={controlId}
      >
        {label}
        {required && !optional && requiredText ? ` · ${requiredText}` : null}
        {optional && optionalText ? ` · ${optionalText}` : null}
      </label>
      {description ? (
        <p
          className={DESCRIPTION_CLASSES}
          id={descriptionId}
        >
          {description}
        </p>
      ) : null}
      {children}
      {status?.message ? (
        <p
          className={mergeClasses(
            STATUS_MESSAGE_CLASSES,
            STATUS_CLASSES[status.type],
          )}
          data-status={status.type}
          data-variant={statusVariant}
          id={statusId}
          aria-live={status.type === "error" ? "assertive" : "polite"}
          role={status.type === "error" ? "alert" : "status"}
        >
          {status.message}
        </p>
      ) : null}
      {disabled && disabledMessage && disabledMessageId ? (
        <p className={DESCRIPTION_CLASSES} id={disabledMessageId}>
          {disabledMessage}
        </p>
      ) : null}
    </section>
  )
}
