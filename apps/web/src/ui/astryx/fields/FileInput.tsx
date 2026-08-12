import {
  type ChangeEvent,
  type DragEventHandler,
  type InputHTMLAttributes,
  type Ref,
  useCallback,
  useId,
  useRef,
  useState,
} from "react"

import {
  CommonFieldProps,
  DESCRIPTION_CLASSES,
  FILE_CLASSES,
  FieldShell,
  FieldStatus,
  FieldStatusVariant,
  mergeClasses,
  mergeDescribedBy,
} from "./shared"

export type FileInputStatus = FieldStatus
export type FileInputStatusVariant = FieldStatusVariant

type NativeFileInputProps = Omit<
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
  | "required"
  | "multiple"
  | "accept"
  | "className"
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

export interface FileInputProps
  extends NativeFileInputProps,
    CommonFieldProps<HTMLInputElement> {
  readonly ref?: Ref<HTMLInputElement>
  readonly label: string
  readonly value: File | File[] | null
  readonly onChange: (files: File | File[] | null) => void
  readonly description?: string
  readonly status?: FileInputStatus
  readonly statusVariant?: FileInputStatusVariant
  readonly htmlName?: string
  readonly accept?: string
  readonly isMultiple?: boolean
  readonly maxSize?: number
  readonly maxFiles?: number
  readonly isLabelHidden?: boolean
  readonly isOptional?: boolean
  readonly isRequired?: boolean
  readonly isDisabled?: boolean
  readonly disabledMessage?: string
  readonly isLoading?: boolean
  readonly placeholder?: string
  readonly mode?: "input" | "dropzone"
  readonly labelTooltip?: string
  readonly clearLabel?: string
  readonly changeAction?: (files: File | File[] | null) => void | Promise<void>
  readonly onDrop?: DragEventHandler<HTMLInputElement>
}

interface ValidationResult {
  readonly valid: File[]
  readonly message?: string
}

function fileSize(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`
  }
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

function matchesAccept(file: File, accept: string): boolean {
  return accept.split(",").some((entry) => {
    const token = entry.trim().toLowerCase()
    if (!token) {
      return true
    }
    if (token.startsWith(".")) {
      return file.name.toLowerCase().endsWith(token)
    }
    if (token.endsWith("/*")) {
      return file.type.toLowerCase().startsWith(token.slice(0, -1))
    }
    return file.type.toLowerCase() === token
  })
}

function validateFiles(
  files: File[],
  accept: string | undefined,
  maxSize: number | undefined,
  maxFiles: number | undefined,
  isMultiple: boolean,
): ValidationResult {
  let valid = files
  let message: string | undefined

  if (accept) {
    const rejected = valid.find((file) => !matchesAccept(file, accept))
    if (rejected) {
      valid = valid.filter((file) => matchesAccept(file, accept))
      message = `"${rejected.name}" is not an accepted file type`
    }
  }

  if (maxSize != null) {
    const rejected = valid.find((file) => file.size > maxSize)
    if (rejected) {
      valid = valid.filter((file) => file.size <= maxSize)
      message ??= `"${rejected.name}" exceeds ${fileSize(maxSize)} limit`
    }
  }

  if (isMultiple && maxFiles != null && valid.length > maxFiles) {
    valid = valid.slice(0, maxFiles)
    message ??= `Maximum ${maxFiles} files allowed`
  }

  return {valid, message}
}

export function FileInput({
  ref: forwardedRef,
  id: providedId,
  className,
  label,
  value,
  onChange,
  description,
  status: statusProp,
  statusVariant = "attached",
  htmlName,
  accept,
  isMultiple = false,
  maxSize,
  maxFiles,
  isLabelHidden = false,
  isOptional = false,
  isRequired = false,
  isDisabled = false,
  disabledMessage,
  isLoading = false,
  placeholder,
  mode = "input",
  labelTooltip,
  clearLabel,
  changeAction,
  onDrop: callerDrop,
  autoComplete,
  translate,
  ["aria-describedby"]: callerDescribedBy,
  ["aria-invalid"]: callerInvalid,
  ["aria-required"]: callerRequired,
  ["aria-disabled"]: callerDisabled,
  ["aria-busy"]: callerBusy,
  ...rest
}: FileInputProps) {
  const generatedId = useId()
  const controlId = providedId ?? generatedId
  const internalRef = useRef<HTMLInputElement | null>(null)
  const [validationMessage, setValidationMessage] = useState<string | undefined>()
  const descriptionId = description ? `${controlId}-description` : undefined
  const effectiveStatus = statusProp ??
    (validationMessage ? {type: "error" as const, message: validationMessage} : undefined)
  const statusId = effectiveStatus?.message ? `${controlId}-status` : undefined
  const disabledMessageId = isDisabled && disabledMessage ? `${controlId}-disabled` : undefined
  const describedBy = mergeDescribedBy(
    callerDescribedBy,
    descriptionId,
    statusId,
    disabledMessageId,
  )
  const hasFocusableDisabledState = isDisabled && Boolean(disabledMessage)
  const selectedFiles = value == null ? [] : Array.isArray(value) ? value : [value]
  const selectedNames = selectedFiles.map((file) => file.name).join(", ")
  const displayText = selectedNames || placeholder || (isMultiple ? "Choose files" : "Choose file")

  const resetNativeInput = useCallback(() => {
    if (internalRef.current) {
      internalRef.current.value = ""
    }
  }, [])

  const handleFiles = useCallback(
    (files: File[]) => {
      if (isDisabled) {
        return
      }
      const result = validateFiles(files, accept, maxSize, maxFiles, isMultiple)
      setValidationMessage(result.message)
      const nextValue = result.valid.length === 0
        ? null
        : isMultiple
          ? result.valid
          : result.valid[0]
      onChange(nextValue)
      if (nextValue != null && changeAction) {
        void changeAction(nextValue)
      }
      resetNativeInput()
    },
    [accept, changeAction, isDisabled, isMultiple, maxFiles, maxSize, onChange, resetNativeInput],
  )

  const handleInputChange = (event: ChangeEvent<HTMLInputElement>) => {
    handleFiles(Array.from(event.currentTarget.files ?? []))
  }

  const handleClear = () => {
    if (isDisabled) {
      return
    }
    setValidationMessage(undefined)
    onChange(null)
    resetNativeInput()
    internalRef.current?.focus()
  }

  const handleDrop: DragEventHandler<HTMLInputElement> = (event) => {
    callerDrop?.(event)
    if (!event.defaultPrevented && mode === "dropzone") {
      event.preventDefault()
      handleFiles(Array.from(event.dataTransfer.files))
    }
  }

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
      labelTooltip={labelTooltip}
      optional={isOptional}
      required={isRequired}
      status={effectiveStatus}
      statusVariant={statusVariant}
    >
      <input
        {...rest}
        ref={(node) => {
          internalRef.current = node
          if (typeof forwardedRef === "function") {
            forwardedRef(node)
          } else if (forwardedRef) {
            forwardedRef.current = node
          }
        }}
        accept={accept}
        aria-busy={isLoading || callerBusy || undefined}
        aria-describedby={describedBy}
        aria-disabled={hasFocusableDisabledState ? true : callerDisabled}
        aria-invalid={effectiveStatus?.type === "error" ? true : callerInvalid}
        aria-required={isRequired && !isOptional ? true : callerRequired}
        autoComplete={autoComplete}
        className={mergeClasses(
          FILE_CLASSES,
          mode === "dropzone" && "min-h-16",
          className,
        )}
        disabled={isDisabled && !hasFocusableDisabledState}
        id={controlId}
        multiple={isMultiple}
        name={htmlName}
        onChange={handleInputChange}
        onDrop={handleDrop}
        required={isRequired && !isOptional}
        type="file"
        translate={translate}
      />
      <p className={DESCRIPTION_CLASSES} data-file-name={selectedNames || undefined}>
        {displayText}
      </p>
      {selectedFiles.length > 0 && !isDisabled && !isLoading ? (
        <button
          aria-label={clearLabel ?? `Clear ${label}`}
          className="justify-self-start rounded-md border border-border-strong px-2 py-1 text-sm text-primary focus-visible:outline focus-visible:outline-accent"
          onClick={handleClear}
          type="button"
        >
          Clear
        </button>
      ) : null}
    </FieldShell>
  )
}

FileInput.displayName = "FileInput"
