import {
  type ChangeEvent,
  type ClipboardEventHandler,
  type KeyboardEventHandler,
  type Ref,
  type TextareaHTMLAttributes,
  useId,
} from "react"

import {
  CommonFieldProps,
  COUNTER_CLASSES,
  FieldShell,
  FieldStatus,
  FieldStatusVariant,
  mergeClasses,
  mergeDescribedBy,
  statusControlClasses,
  TEXTAREA_CLASSES,
} from "./shared"

export type TextAreaStatus = FieldStatus
export type TextAreaStatusVariant = FieldStatusVariant

type NativeTextAreaProps = Omit<
  TextareaHTMLAttributes<HTMLTextAreaElement>,
  | "id"
  | "name"
  | "value"
  | "defaultValue"
  | "onChange"
  | "disabled"
  | "readOnly"
  | "required"
  | "autoFocus"
  | "className"
  | "maxLength"
  | "rows"
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

export interface TextAreaProps
  extends NativeTextAreaProps,
    CommonFieldProps<HTMLTextAreaElement> {
  readonly ref?: Ref<HTMLTextAreaElement>
  readonly label: string
  readonly value: string
  readonly onChange?: (value: string, event: ChangeEvent<HTMLTextAreaElement>) => void
  readonly description?: string
  readonly status?: TextAreaStatus
  readonly statusVariant?: TextAreaStatusVariant
  readonly htmlName?: string
  readonly maxLength?: number
  readonly rows?: number
  readonly isLabelHidden?: boolean
  readonly isOptional?: boolean
  readonly isRequired?: boolean
  readonly requiredText?: string
  readonly optionalText?: string
  readonly isDisabled?: boolean
  readonly disabledMessage?: string
  readonly isLoading?: boolean
  readonly hasSpellCheck?: boolean
  readonly hasAutoFocus?: boolean
  readonly onPaste?: ClipboardEventHandler<HTMLTextAreaElement>
  readonly onKeyDown?: KeyboardEventHandler<HTMLTextAreaElement>
}

function assignRef<T>(ref: Ref<T> | undefined, value: T | null): void {
  if (typeof ref === "function") {
    ref(value)
  } else if (ref) {
    ref.current = value
  }
}

export function TextArea({
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
  maxLength,
  rows = 3,
  isLabelHidden = false,
  isOptional = false,
  isRequired = false,
  requiredText,
  optionalText,
  isDisabled = false,
  disabledMessage,
  isLoading = false,
  hasSpellCheck = true,
  hasAutoFocus = false,
  onPaste,
  onKeyDown,
  autoComplete,
  translate,
  ["aria-describedby"]: callerDescribedBy,
  ["aria-invalid"]: callerInvalid,
  ["aria-required"]: callerRequired,
  ["aria-disabled"]: callerDisabled,
  ["aria-busy"]: callerBusy,
  ...rest
}: TextAreaProps) {
  const generatedId = useId()
  const controlId = providedId ?? generatedId
  const descriptionId = description ? `${controlId}-description` : undefined
  const statusId = status?.message ? `${controlId}-status` : undefined
  const counterId = maxLength != null ? `${controlId}-counter` : undefined
  const disabledMessageId = isDisabled && disabledMessage ? `${controlId}-disabled` : undefined
  const describedBy = mergeDescribedBy(
    callerDescribedBy,
    descriptionId,
    statusId,
    counterId,
    disabledMessageId,
  )
  const effectiveValue = value ?? ""
  const overLimit = maxLength != null && effectiveValue.length > maxLength

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
      <textarea
        {...rest}
        ref={(node) => {
          assignRef(forwardedRef, node)
        }}
        aria-busy={isLoading || callerBusy || undefined}
        aria-describedby={describedBy}
        aria-disabled={isDisabled ? true : callerDisabled}
        aria-invalid={overLimit || status?.type === "error" ? true : callerInvalid}
        aria-required={isRequired && !isOptional ? true : callerRequired}
        autoComplete={autoComplete}
        autoFocus={hasAutoFocus}
        className={mergeClasses(
          TEXTAREA_CLASSES,
          statusControlClasses(status),
          overLimit && "border-error",
          className,
        )}
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
        onKeyDown={onKeyDown}
        onPaste={onPaste}
        placeholder={rest.placeholder}
        required={isRequired && !isOptional}
        rows={rows}
        spellCheck={hasSpellCheck}
        translate={translate}
        value={effectiveValue}
      />
      {maxLength != null ? (
        <p
          className={mergeClasses(COUNTER_CLASSES, overLimit && "text-error")}
          id={counterId}
        >
          {effectiveValue.length}/{maxLength}
        </p>
      ) : null}
    </FieldShell>
  )
}

TextArea.displayName = "TextArea"
