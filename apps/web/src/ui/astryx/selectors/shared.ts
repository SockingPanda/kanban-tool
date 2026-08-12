import type { ReactNode } from "react"

import type {CspSafeDomProps} from "../dom-props"

export type SelectorOptionData = {
  readonly value: string
  readonly label?: string
  readonly disabled?: boolean
  readonly icon?: ReactNode
}

/** The option shape that the native Selector can represent without loss. */
export type NativeSelectorOptionData = {
  readonly value: string
  readonly label?: string
  readonly disabled?: boolean
}

export type NativeSelectorSection = {
  readonly type: "section"
  readonly title?: string
  readonly options: readonly NativeSelectorOptionData[]
}

export type NativeSelectorOption = string | NativeSelectorOptionData | NativeSelectorSection
export type NativeSelectorOptionType = NativeSelectorOption

export type NativeSelectableOption = {
  readonly value: string
  readonly label: string
  readonly disabled?: boolean
}

export type SelectorDivider = { readonly type: "divider" }

export type SelectorSection = {
  readonly type: "section"
  readonly title?: string
  readonly options: readonly SelectorOptionData[]
}

export type SelectorOption = string | SelectorOptionData | SelectorDivider | SelectorSection
export type SelectorOptionType = SelectorOption

export type SelectableOption = {
  readonly value: string
  readonly label: string
  readonly disabled?: boolean
  readonly icon?: ReactNode
}

export type SelectorStatusType = "warning" | "error" | "success"

export type SelectorStatus = {
  readonly type: SelectorStatusType
  readonly message?: string
}

/** Shared data/ARIA surface for controls without inheriting presentation-bearing HTML props. */
export type SafeDomProps = CspSafeDomProps<HTMLElement> & {
  readonly id?: string
  readonly className?: string
}

export function isDivider(option: SelectorOption): option is SelectorDivider {
  return typeof option === "object" && "type" in option && option.type === "divider"
}

export function isSection(option: SelectorOption): option is SelectorSection {
  return typeof option === "object" && "type" in option && option.type === "section"
}

export function isOptionData(option: SelectorOption): option is SelectorOptionData {
  return typeof option === "string" || (typeof option === "object" && !isDivider(option) && !isSection(option))
}

export function normalizeOption(option: string | SelectorOptionData): SelectableOption {
  if (typeof option === "string") {
    return { value: option, label: option }
  }
  return {
    value: option.value,
    label: option.label ?? option.value,
    disabled: option.disabled,
    icon: option.icon,
  }
}

export function flattenOptions(options: readonly SelectorOption[]): readonly SelectableOption[] {
  const flattened: SelectableOption[] = []
  for (const option of options) {
    if (isSection(option)) {
      for (const child of option.options) {
        flattened.push(normalizeOption(child))
      }
    } else if (isOptionData(option)) {
      flattened.push(normalizeOption(option))
    }
  }
  return flattened
}

export function optionGroups(options: readonly SelectorOption[]): readonly {
  readonly title?: string
  readonly options: readonly SelectableOption[]
}[] {
  const groups: { title?: string; options: readonly SelectableOption[] }[] = []
  let loose: SelectableOption[] = []
  const flushLoose = () => {
    if (loose.length > 0) {
      groups.push({ options: loose })
      loose = []
    }
  }
  for (const option of options) {
    if (isSection(option)) {
      flushLoose()
      groups.push({ title: option.title, options: option.options.map(normalizeOption) })
    } else if (isOptionData(option)) {
      loose.push(normalizeOption(option))
    }
  }
  flushLoose()
  return groups
}

function hasUnsupportedNativeFields(option: object): boolean {
  return "icon" in option && (option as { readonly icon?: unknown }).icon !== undefined
}

function normalizeNativeOption(option: string | NativeSelectorOptionData): NativeSelectableOption {
  if (typeof option === "string") {
    return { value: option, label: option }
  }
  if (hasUnsupportedNativeFields(option)) {
    throw new TypeError("Selector native options do not support icons")
  }
  return {
    value: option.value,
    label: option.label ?? option.value,
    disabled: option.disabled,
  }
}

/** Native `<select>` groups; unsupported divider/icon shapes fail loudly at runtime. */
export function nativeOptionGroups(options: readonly NativeSelectorOption[]): readonly {
  readonly title?: string
  readonly options: readonly NativeSelectableOption[]
}[] {
  const groups: { title?: string; options: readonly NativeSelectableOption[] }[] = []
  let loose: NativeSelectableOption[] = []
  const flushLoose = () => {
    if (loose.length > 0) {
      groups.push({ options: loose })
      loose = []
    }
  }
  for (const option of options) {
    if (typeof option === "object" && "type" in option && option.type === "section") {
      flushLoose()
      groups.push({
        title: option.title,
        options: option.options.map(normalizeNativeOption),
      })
    } else if (typeof option === "object") {
      if ("type" in option) throw new TypeError("Selector native options do not support dividers")
      loose.push(normalizeNativeOption(option))
    } else {
      loose.push(normalizeNativeOption(option))
    }
  }
  flushLoose()
  return groups
}

export function joinIds(...ids: readonly (string | undefined)[]): string | undefined {
  const seen = new Set<string>()
  const present: string[] = []
  for (const value of ids) {
    for (const id of value?.split(/\s+/) ?? []) {
      if (id.length > 0 && !seen.has(id)) {
        seen.add(id)
        present.push(id)
      }
    }
  }
  return present.length === 0 ? undefined : present.join(" ")
}

export function mergeClasses(...classes: readonly (string | false | null | undefined)[]): string | undefined {
  const merged = classes.filter((value): value is string => Boolean(value)).join(" ")
  return merged.length > 0 ? merged : undefined
}

export const fieldClass = "relative flex min-w-0 flex-col gap-1"
export const labelClass = "text-sm font-medium text-primary"
export const hiddenLabelClass = "text-sm font-medium text-primary sr-only"
export const descriptionClass = "text-xs text-secondary"
export const statusClasses = {
  attached: "text-xs",
  detached: "mt-1 text-xs",
} as const

export const selectorControlClasses = {
  input: {
    sm: "min-h-7 rounded-md border border-border bg-surface px-2 py-1 text-sm text-primary outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-60",
    md: "min-h-8 rounded-md border border-border bg-surface px-3 py-1.5 text-sm text-primary outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-60",
    lg: "min-h-9 rounded-md border border-border bg-surface px-4 py-2 text-sm text-primary outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-60",
  },
  ghost: {
    sm: "min-h-7 rounded-md border border-transparent bg-transparent px-2 py-1 text-sm text-primary outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-60",
    md: "min-h-8 rounded-md border border-transparent bg-transparent px-3 py-1.5 text-sm text-primary outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-60",
    lg: "min-h-9 rounded-md border border-transparent bg-transparent px-4 py-2 text-sm text-primary outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-60",
  },
} as const

export const selectorTriggerClasses = {
  input: {
    sm: "flex min-h-7 w-full items-center justify-between gap-2 rounded-md border border-border bg-surface px-2 py-1 text-left text-sm text-primary outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-60",
    md: "flex min-h-8 w-full items-center justify-between gap-2 rounded-md border border-border bg-surface px-3 py-1.5 text-left text-sm text-primary outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-60",
    lg: "flex min-h-9 w-full items-center justify-between gap-2 rounded-md border border-border bg-surface px-4 py-2 text-left text-sm text-primary outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-60",
  },
  ghost: {
    sm: "flex min-h-7 w-full items-center justify-between gap-2 rounded-md border border-transparent bg-transparent px-2 py-1 text-left text-sm text-primary outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-60",
    md: "flex min-h-8 w-full items-center justify-between gap-2 rounded-md border border-transparent bg-transparent px-3 py-1.5 text-left text-sm text-primary outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-60",
    lg: "flex min-h-9 w-full items-center justify-between gap-2 rounded-md border border-transparent bg-transparent px-4 py-2 text-left text-sm text-primary outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-60",
  },
} as const

export const inputClasses = {
  sm: "min-h-7 w-full rounded-md border border-border bg-surface px-2 py-1 text-sm text-primary outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-60",
  md: "min-h-8 w-full rounded-md border border-border bg-surface px-3 py-1.5 text-sm text-primary outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-60",
  lg: "min-h-9 w-full rounded-md border border-border bg-surface px-4 py-2 text-sm text-primary outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-60",
} as const

export const listboxClass = "mt-1 max-h-60 overflow-auto rounded-md border border-border bg-popover p-1 shadow-sm"
export const optionClass = "flex w-full cursor-default items-center gap-2 rounded px-2 py-1.5 text-left text-sm text-primary"
export const groupClass = "flex flex-col gap-0.5"
export const srOnlyClass = "sr-only"
