import {
  type ChangeEvent,
  type InputHTMLAttributes,
  type KeyboardEvent,
  type Ref,
  useId,
  useRef,
} from "react"

import {
  CONTROL_CLASSES,
  CommonFieldProps,
  FieldShell,
  FieldStatus,
  FieldStatusVariant,
  mergeClasses,
  mergeDescribedBy,
  statusControlClasses,
} from "./shared"

export type TextInputType = "text" | "password" | "email" | "search" | "url" | "tel"
export type TextInputStatus = FieldStatus
export type TextInputStatusVariant = FieldStatusVariant

type NativeTextInputProps = Omit<
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
  | "maxLength"
  | "aria-describedby"
  | "aria-invalid"
  | "aria-required"
  | "aria-disabled"
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

interface TextInputBaseProps
  extends NativeTextInputProps,
    CommonFieldProps<HTMLInputElement> {
  readonly ref?: Ref<HTMLInputElement>
  readonly type?: TextInputType
  readonly label: string
  readonly value: string
  readonly onChange?: (value: string, event: ChangeEvent<HTMLInputElement> | null) => void
  readonly description?: string
  readonly status?: TextInputStatus
  readonly statusVariant?: TextInputStatusVariant
  readonly htmlName?: string
  readonly maxLength?: number
  readonly isLabelHidden?: boolean
  readonly isOptional?: boolean
  readonly isRequired?: boolean
  readonly requiredText?: string
  readonly optionalText?: string
  readonly isDisabled?: boolean
  readonly disabledMessage?: string
  readonly isLoading?: boolean
  readonly placeholder?: string
  readonly hasAutoFocus?: boolean
  readonly onEnter?: () => void
}

export type TextInputProps =
  | (TextInputBaseProps & {
      readonly hasClear?: false
      readonly clearLabel?: never
      readonly clearText?: never
    })
  | (TextInputBaseProps & {
      readonly hasClear: true
      readonly clearLabel: string
      readonly clearText: string
    })

function assignRef<T>(ref: Ref<T> | undefined, value: T | null): void {
  if (typeof ref === "function") {
    ref(value)
  } else if (ref) {
    ref.current = value
  }
}

export function TextInput({
  ref: forwardedRef,
  id: providedId,
  className,
  type = "text",
  label,
  value,
  onChange,
  description,
  status,
  statusVariant = "attached",
  htmlName,
  maxLength,
  isLabelHidden = false,
  isOptional = false,
  isRequired = false,
  requiredText,
  optionalText,
  isDisabled = false,
  disabledMessage,
  isLoading = false,
  placeholder,
  hasClear = false,
  clearLabel,
  clearText,
  hasAutoFocus = false,
  onEnter,
  onKeyDown,
  autoComplete,
  translate,
  ["aria-describedby"]: callerDescribedBy,
  ["aria-invalid"]: callerInvalid,
  ["aria-required"]: callerRequired,
  ["aria-disabled"]: callerDisabled,
  ["aria-busy"]: callerBusy,
  ...rest
}: TextInputProps) {
  const generatedId = useId()
  const inputRef = useRef<HTMLInputElement | null>(null)
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
  const effectiveValue = value ?? ""

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter") {
      onEnter?.()
    }
    onKeyDown?.(event)
  }

  const input = (
    <input
      {...rest}
      ref={(node) => {
        inputRef.current = node
        assignRef(forwardedRef, node)
      }}
      aria-busy={isLoading || callerBusy || undefined}
      aria-describedby={describedBy}
      aria-disabled={isDisabled ? true : callerDisabled}
      aria-invalid={status?.type === "error" ? true : callerInvalid}
      aria-required={isRequired && !isOptional ? true : callerRequired}
      autoComplete={autoComplete}
      autoFocus={hasAutoFocus}
      className={mergeClasses(CONTROL_CLASSES, statusControlClasses(status), className)}
      disabled={isDisabled}
      id={controlId}
      maxLength={maxLength}
      name={htmlName}
      onChange={(event) => {
        if (isDisabled) {
          return
        }
        onChange?.(event.target.value, event)
      }}
      onKeyDown={onEnter || onKeyDown ? handleKeyDown : undefined}
      placeholder={placeholder}
      required={isRequired && !isOptional}
      type={type}
      translate={translate}
      value={effectiveValue}
    />
  )

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
      {input}
      {hasClear && effectiveValue && !isDisabled ? (
        <button
          aria-label={clearLabel}
          className="justify-self-start rounded-md border border-border-strong px-2 py-1 text-sm text-primary focus-visible:outline focus-visible:outline-accent disabled:cursor-not-allowed disabled:opacity-50"
          onClick={(event) => {
            event.preventDefault()
            if (!isDisabled) {
              onChange?.("", null)
              inputRef.current?.focus()
            }
          }}
          type="button"
        >
          {clearText}
        </button>
      ) : null}
    </FieldShell>
  )
}

TextInput.displayName = "TextInput"
