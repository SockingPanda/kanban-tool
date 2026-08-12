import { forwardRef, useId, useState, type ChangeEvent, type Ref } from "react"

import {
  descriptionClass,
  fieldClass,
  hiddenLabelClass,
  joinIds,
  labelClass,
  mergeClasses,
  nativeOptionGroups,
  selectorControlClasses,
  statusClasses,
  type SafeDomProps,
  type NativeSelectableOption,
  type NativeSelectorOption,
  type SelectorStatus,
} from "./shared"

export type {
  NativeSelectorOption as SelectorOption,
  NativeSelectorOptionData as SelectorOptionData,
  NativeSelectorOptionType as SelectorOptionType,
  NativeSelectorSection as SelectorSection,
  SelectorStatus,
} from "./shared"

type SelectorPropsBase = SafeDomProps & {
  readonly label: string
  readonly description?: string
  readonly status?: SelectorStatus
  readonly statusVariant?: "attached" | "detached"
  readonly htmlName?: string
  readonly options: readonly NativeSelectorOption[]
  readonly value?: string
  readonly defaultValue?: string
  readonly onChange?: (value: string) => void
  readonly renderOption?: (option: NativeSelectableOption) => string
  /** Explicit empty option copy; an omitted value is not a valid no-selection state. */
  readonly placeholder: string
  readonly isLabelHidden?: boolean
  readonly isDisabled?: boolean
  readonly isLoading?: boolean
  readonly loadingText: string
  readonly size?: "sm" | "md" | "lg"
  readonly variant?: "input" | "ghost"
  readonly required?: boolean
}

type SelectorRequirementProps =
  | { readonly isRequired: true; readonly isOptional?: false; readonly optionalLabel?: never }
  | { readonly isRequired?: false; readonly isOptional: true; readonly optionalLabel: string }
  | { readonly isRequired?: false; readonly isOptional?: false; readonly optionalLabel?: never }

export type SelectorProps = SelectorPropsBase & SelectorRequirementProps

function optionContent(option: NativeSelectableOption, renderOption?: (option: NativeSelectableOption) => string): string {
  return renderOption ? renderOption(option) : option.label
}

function SelectorImpl(
  {
    label,
    description,
    status,
    statusVariant = "attached",
    htmlName,
    options,
    value,
    defaultValue,
    onChange,
    renderOption,
    placeholder,
    isLabelHidden = false,
    isOptional = false,
    optionalLabel,
    isRequired = false,
    isDisabled = false,
    isLoading = false,
    loadingText,
    size = "md",
    variant = "input",
    id,
    className,
    tabIndex,
    onBlur,
    onFocus,
    onKeyDown,
    onMouseDown,
    onClick,
    required,
    "data-testid": testId,
    ...domProps
  }: SelectorProps,
  ref: Ref<HTMLSelectElement>,
) {
  const generatedId = useId().replaceAll(":", "")
  const controlId = id ?? `selector-${generatedId}`
  const descriptionId = description ? `${controlId}-description` : undefined
  const statusId = status?.message ? `${controlId}-status` : undefined
  const [internalValue, setInternalValue] = useState(defaultValue ?? "")
  const selectedValue = value === undefined ? internalValue : value
  const describedBy = joinIds(domProps["aria-describedby"], descriptionId, statusId)
  const disabled = isDisabled || isLoading
  const nativeRequired = required ?? isRequired
  const groups = nativeOptionGroups(options)

  const handleChange = (event: ChangeEvent<HTMLSelectElement>) => {
    const nextValue = event.currentTarget.value
    if (value === undefined) {
      setInternalValue(nextValue)
    }
    onChange?.(nextValue)
  }

  return (
    <section
      className={mergeClasses(fieldClass, className)}
      aria-busy={isLoading || undefined}
      data-status={status?.type}
    >
      <label className={isLabelHidden ? hiddenLabelClass : labelClass} htmlFor={controlId}>
        {label}
        {nativeRequired ? <small aria-hidden="true"> *</small> : null}
        {isOptional ? <small aria-hidden="true"> ({optionalLabel})</small> : null}
      </label>
      {description ? <p id={descriptionId} className={descriptionClass}>{description}</p> : null}
      <select
        {...domProps}
        ref={ref}
        id={controlId}
        className={selectorControlClasses[variant][size]}
        name={htmlName}
        value={selectedValue}
        disabled={disabled}
        required={nativeRequired}
        tabIndex={tabIndex}
        aria-describedby={describedBy}
        aria-required={nativeRequired ? true : domProps["aria-required"]}
        aria-disabled={disabled ? true : domProps["aria-disabled"]}
        aria-invalid={status?.type === "error" ? true : domProps["aria-invalid"]}
        aria-busy={isLoading || domProps["aria-busy"]}
        data-selector-control="true"
        onBlur={onBlur}
        onFocus={onFocus}
        onKeyDown={onKeyDown}
        onMouseDown={onMouseDown}
        onClick={onClick}
        onChange={handleChange}
        data-testid={testId}
      >
        <option value="">{placeholder}</option>
        {groups.map((group, groupIndex) => {
          const children = group.options.map((option) => (
            <option key={option.value} value={option.value} disabled={option.disabled}>
              {optionContent(option, renderOption)}
            </option>
          ))
          return group.title === undefined ? children : (
            <optgroup key={`section-${groupIndex}`} label={group.title}>
              {children}
            </optgroup>
          )
        })}
      </select>
      {status?.message ? (
        <p
          id={statusId}
          className={statusClasses[statusVariant]}
          role={status.type === "error" ? "alert" : "status"}
        >
          {status.message}
        </p>
      ) : null}
      {isLoading ? <output className="text-xs text-secondary" role="status" aria-live="polite">{loadingText}</output> : null}
    </section>
  )
}

export const Selector = forwardRef<HTMLSelectElement, SelectorProps>(SelectorImpl)
Selector.displayName = "Selector"

export default Selector
