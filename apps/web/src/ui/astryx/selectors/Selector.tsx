import { forwardRef, useId, useState, type ChangeEvent, type Ref } from "react"

import {
  descriptionClass,
  fieldClass,
  hiddenLabelClass,
  joinIds,
  labelClass,
  mergeClasses,
  optionGroups,
  selectorControlClasses,
  statusClasses,
  type SafeDomProps,
  type SelectableOption,
  type SelectorOption,
  type SelectorStatus,
} from "./shared"

export type { SelectorOptionData, SelectorOptionType, SelectorOption, SelectorStatus } from "./shared"

type SelectorPropsBase = SafeDomProps & {
  readonly label: string
  readonly description?: string
  readonly status?: SelectorStatus
  readonly statusVariant?: "attached" | "detached"
  readonly htmlName?: string
  readonly options: readonly SelectorOption[]
  readonly value?: string
  readonly defaultValue?: string
  readonly onChange?: (value: string) => void
  readonly renderOption?: (option: SelectableOption) => string
  readonly placeholder?: string
  readonly isLabelHidden?: boolean
  readonly isRequired?: boolean
  readonly isDisabled?: boolean
  readonly isLoading?: boolean
  readonly loadingText: string
  readonly size?: "sm" | "md" | "lg"
  readonly variant?: "input" | "ghost"
  readonly required?: boolean
}

export type SelectorProps = SelectorPropsBase &
  ({ readonly isOptional: true; readonly optionalLabel: string } | { readonly isOptional?: false; readonly optionalLabel?: never })

function optionContent(option: SelectableOption, renderOption?: (option: SelectableOption) => string): string {
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
  const descriptionId = description === undefined ? undefined : `${controlId}-description`
  const statusId = status?.message === undefined ? undefined : `${controlId}-status`
  const [internalValue, setInternalValue] = useState(defaultValue ?? "")
  const selectedValue = value === undefined ? internalValue : value
  const describedBy = joinIds(domProps["aria-describedby"], descriptionId, statusId)
  const disabled = isDisabled || isLoading
  const groups = optionGroups(options)
  const loadingTextValue = loadingText

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
      data-testid={testId}
      data-status={status?.type}
    >
      <label className={isLabelHidden ? hiddenLabelClass : labelClass} htmlFor={controlId}>
        {label}
        {isRequired ? <small aria-hidden="true"> *</small> : null}
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
        required={required ?? isRequired}
        tabIndex={tabIndex}
        aria-describedby={describedBy}
        aria-required={isRequired ? true : domProps["aria-required"]}
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
      >
        {placeholder ? <option value="">{placeholder}</option> : null}
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
      {isLoading ? <output className="text-xs text-secondary" role="status">{loadingTextValue}</output> : null}
    </section>
  )
}

export const Selector = forwardRef<HTMLSelectElement, SelectorProps>(SelectorImpl)
Selector.displayName = "Selector"

export default Selector
