import {
  type ChangeEvent,
  type FocusEventHandler,
  type InputHTMLAttributes,
  type Ref,
  useId,
} from "react"

import {
  CHECKBOX_CLASSES,
  CommonFieldProps,
  FieldShell,
  FieldStatus,
  FieldStatusVariant,
  mergeClasses,
  mergeDescribedBy,
  statusControlClasses,
} from "./shared"

export type CheckboxInputSize = "sm" | "md"
export type CheckboxInputStatus = FieldStatus
export type CheckboxInputStatusVariant = FieldStatusVariant

type NativeCheckboxProps = Omit<
  InputHTMLAttributes<HTMLInputElement>,
  | "id"
  | "name"
  | "type"
  | "value"
  | "defaultValue"
  | "checked"
  | "defaultChecked"
  | "onChange"
  | "disabled"
  | "readOnly"
  | "required"
  | "autoFocus"
  | "className"
  | "size"
  | "aria-describedby"
  | "aria-invalid"
  | "aria-required"
  | "aria-disabled"
  | "aria-readonly"
  | "aria-busy"
  | "autoComplete"
  | "translate"
  | "onFocus"
  | "onBlur"
  | "onKeyDown"
  | "onKeyUp"
  | "onClick"
  | "style"
  | "width"
>

export interface CheckboxInputProps
  extends NativeCheckboxProps,
    CommonFieldProps<HTMLInputElement> {
  readonly ref?: Ref<HTMLInputElement>
  readonly label: string
  readonly value: boolean | "indeterminate"
  readonly onChange?: (checked: boolean, event: ChangeEvent<HTMLInputElement>) => void
  readonly description?: string
  readonly status?: CheckboxInputStatus
  readonly statusVariant?: CheckboxInputStatusVariant
  readonly htmlName?: string
  readonly size?: CheckboxInputSize
  readonly isLabelHidden?: boolean
  readonly isOptional?: boolean
  readonly isRequired?: boolean
  readonly requiredText?: string
  readonly optionalText?: string
  readonly isDisabled?: boolean
  readonly disabledMessage?: string
  readonly isReadOnly?: boolean
  readonly isLoading?: boolean
  readonly onFocus?: FocusEventHandler<HTMLInputElement>
  readonly onBlur?: FocusEventHandler<HTMLInputElement>
}

function assignRef<T>(ref: Ref<T> | undefined, value: T | null): void {
  if (typeof ref === "function") {
    ref(value)
  } else if (ref) {
    ref.current = value
  }
}

export function CheckboxInput({
  ref: forwardedRef,
  id: providedId,
  className,
  label,
  value,
  onChange,
  description,
  status,
  statusVariant = "attached",
  htmlName,
  size = "md",
  isLabelHidden = false,
  isOptional = false,
  isRequired = false,
  requiredText,
  optionalText,
  isDisabled = false,
  disabledMessage,
  isReadOnly = false,
  isLoading = false,
  onFocus,
  onBlur,
  autoComplete,
  translate,
  ["aria-describedby"]: callerDescribedBy,
  ["aria-invalid"]: callerInvalid,
  ["aria-required"]: callerRequired,
  ["aria-disabled"]: callerDisabled,
  ["aria-readonly"]: callerReadonly,
  ["aria-busy"]: callerBusy,
  ...rest
}: CheckboxInputProps) {
  const generatedId = useId()
  const controlId = providedId ?? generatedId
  const descriptionId = description ? `${controlId}-description` : undefined
  const statusId = status?.message ? `${controlId}-status` : undefined
  const disabledMessageId = isDisabled && disabledMessage ? `${controlId}-disabled` : undefined
  const describedBy = mergeDescribedBy(
    callerDescribedBy,
    descriptionId,
    statusId,
    disabledMessageId,
  )
  const isIndeterminate = value === "indeterminate"

  return (
    <FieldShell
      className="w-full"
      controlId={controlId}
      description={description}
      descriptionId={descriptionId}
      disabled={isDisabled}
      disabledMessage={disabledMessage}
      disabledMessageId={disabledMessageId}
      label={label}
      labelHidden={isLabelHidden}
      optionalText={optionalText}
      optional={isOptional}
      required={isRequired}
      requiredText={requiredText}
      status={status}
      statusVariant={statusVariant}
    >
      <input
        {...rest}
        ref={(node) => {
          if (node) {
            node.indeterminate = isIndeterminate
          }
          assignRef(forwardedRef, node)
        }}
        aria-busy={isLoading || callerBusy || undefined}
        aria-describedby={describedBy}
        aria-disabled={isDisabled ? true : callerDisabled}
        aria-invalid={status?.type === "error" ? true : callerInvalid}
        aria-readonly={isReadOnly || callerReadonly || undefined}
        aria-required={isRequired && !isOptional ? true : callerRequired}
        autoComplete={autoComplete}
        checked={value === true}
        className={mergeClasses(
          CHECKBOX_CLASSES,
          size === "sm" ? "h-4 w-4" : undefined,
          statusControlClasses(status),
          className,
        )}
        disabled={isDisabled}
        id={controlId}
        name={htmlName}
        onBlur={onBlur}
        onChange={(event) => {
          if (isDisabled || isReadOnly || isLoading) {
            return
          }
          onChange?.(event.target.checked, event)
        }}
        onFocus={onFocus}
        readOnly={isReadOnly}
        required={isRequired && !isOptional}
        type="checkbox"
        translate={translate}
      />
    </FieldShell>
  )
}

CheckboxInput.displayName = "CheckboxInput"
