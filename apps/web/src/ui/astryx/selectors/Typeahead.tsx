import {
  forwardRef,
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
  type ChangeEvent,
  type FocusEvent,
  type KeyboardEvent,
  type ReactNode,
  type Ref,
  type ReactElement,
} from "react"

import {
  descriptionClass,
  fieldClass,
  inputClasses,
  hiddenLabelClass,
  joinIds,
  labelClass,
  listboxClass,
  mergeClasses,
  optionClass,
  statusClasses,
  type SafeDomProps,
  type SelectorStatus,
} from "./shared"
import type { SearchableItem, SearchSource } from "./search-source"

export type { SearchableItem, SearchSource } from "./search-source"

type TypeaheadPropsBase<T extends SearchableItem> = SafeDomProps & {
  readonly label: string
  readonly description?: string
  readonly status?: SelectorStatus
  readonly statusVariant?: "attached" | "detached"
  readonly htmlName?: string
  readonly searchSource: SearchSource<T>
  readonly value: T | null
  readonly onChange: (item: T | null) => void
  readonly renderItem?: (item: T) => ReactNode
  readonly renderOption?: (item: T) => ReactNode
  readonly placeholder: string
  readonly searchLabel: string
  readonly loadingText: string
  readonly clearLabel: string
  readonly hasEntriesOnFocus?: boolean
  readonly maxMenuItems?: number
  readonly emptySearchResultsText: string
  readonly errorText: string
  readonly isDisabled?: boolean
  readonly disabledMessage?: string
  readonly hasClear?: boolean
  readonly hasAutoFocus?: boolean
  readonly isLabelHidden?: boolean
  readonly listboxLabel: string
  readonly required?: boolean
  readonly size?: "sm" | "md" | "lg"
  readonly debounceMs?: number
  readonly onChangeQuery?: (query: string) => void
  readonly onOpenChange?: (open: boolean) => void
  readonly inputId?: string
  readonly listboxId?: string
  readonly "data-testid"?: string
}

type TypeaheadRequirementProps =
  | { readonly isRequired: true; readonly isOptional?: false; readonly optionalLabel?: never }
  | { readonly isRequired?: false; readonly isOptional: true; readonly optionalLabel: string }
  | { readonly isRequired?: false; readonly isOptional?: false; readonly optionalLabel?: never }

export type TypeaheadProps<T extends SearchableItem> = TypeaheadPropsBase<T> & TypeaheadRequirementProps

function TypeaheadImpl<T extends SearchableItem>(
  {
    label,
    description,
    status,
    statusVariant = "attached",
    htmlName,
    searchSource,
    value,
    onChange,
    renderItem,
    renderOption,
    placeholder,
    searchLabel,
    loadingText,
    clearLabel,
    optionalLabel,
    hasEntriesOnFocus = false,
    maxMenuItems = 10,
    emptySearchResultsText,
    errorText,
    isDisabled = false,
    disabledMessage,
    hasClear = true,
    hasAutoFocus = false,
    isLabelHidden = false,
    isOptional = false,
    isRequired = false,
    required,
    listboxLabel,
    size = "md",
    debounceMs = 150,
    onChangeQuery,
    onOpenChange,
    inputId: inputIdProp,
    listboxId: listboxIdProp,
    id,
    className,
    tabIndex,
    onBlur,
    onFocus,
    onKeyDown,
    onMouseDown,
    onClick,
    "data-testid": testId,
    ...domProps
  }: TypeaheadProps<T>,
  forwardedRef: Ref<HTMLInputElement>,
) {
  const generatedId = useId().replaceAll(":", "")
  const inputId = inputIdProp ?? id ?? `typeahead-${generatedId}`
  const listboxId = listboxIdProp ?? `${inputId}-listbox`
  const descriptionId = description ? `${inputId}-description` : undefined
  const statusId = status?.message ? `${inputId}-status` : undefined
  const disabledMessageId = !isDisabled || !disabledMessage ? undefined : `${inputId}-disabled-message`
  const placeholderText = placeholder
  const loadingTextValue = loadingText
  const clearText = clearLabel
  const optionalText = optionalLabel
  const noResultsText = emptySearchResultsText
  const errorTextValue = errorText
  const searchText = searchLabel
  const resultsText = listboxLabel
  const inputRef = useRef<HTMLInputElement>(null)
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  const generationRef = useRef(0)
  const [query, setQuery] = useState("")
  const [results, setResults] = useState<readonly T[]>([])
  const [activeIndex, setActiveIndex] = useState(-1)
  const [open, setOpen] = useState(false)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | undefined>(undefined)
  const [failed, setFailed] = useState(false)
  const openRef = useRef(open)
  const onOpenChangeRef = useRef(onOpenChange)
  openRef.current = open
  onOpenChangeRef.current = onOpenChange
  const errorId = failed && error ? `${inputId}-error` : undefined
  const describedBy = joinIds(domProps["aria-describedby"], descriptionId, statusId, errorId, disabledMessageId)

  const setInputRef = (node: HTMLInputElement | null) => {
    inputRef.current = node
    if (typeof forwardedRef === "function") forwardedRef(node)
    else if (forwardedRef) forwardedRef.current = node
  }

  const invalidatePending = useCallback(() => {
    generationRef.current += 1
    if (timeoutRef.current !== undefined) clearTimeout(timeoutRef.current)
    timeoutRef.current = undefined
    searchSource.cancel?.()
  }, [searchSource])

  const setOpenState = useCallback((next: boolean) => {
    if (openRef.current === next) {
      if (!next) invalidatePending()
      return
    }
    openRef.current = next
    setOpen(next)
    onOpenChangeRef.current?.(next)
    if (!next) {
      setActiveIndex(-1)
      setLoading(false)
      invalidatePending()
    }
  }, [invalidatePending])

  const clearQuery = useCallback((close = true) => {
    invalidatePending()
    setQuery("")
    onChangeQuery?.("")
    setResults([])
    setError(undefined)
    setFailed(false)
    setLoading(false)
    if (close) setOpenState(false)
  }, [invalidatePending, onChangeQuery, setOpenState])

  const runSearch = useCallback(async (nextQuery: string) => {
    if (isDisabled) return
    const generation = ++generationRef.current
    searchSource.cancel?.()
    setLoading(true)
    setError(undefined)
    setFailed(false)
    setOpenState(true)
    try {
      const found = await searchSource.search(nextQuery)
      if (generationRef.current !== generation) return
      const shown = found.slice(0, maxMenuItems)
      setResults(shown)
      setActiveIndex(shown.length > 0 ? 0 : -1)
    } catch {
      if (generationRef.current !== generation) return
      setResults([])
      setActiveIndex(-1)
      setError(errorTextValue)
      setFailed(true)
    } finally {
      if (generationRef.current === generation) setLoading(false)
    }
  }, [errorTextValue, isDisabled, maxMenuItems, searchSource, setOpenState])

  const runBootstrap = useCallback(async () => {
    if (isDisabled) return
    const generation = ++generationRef.current
    searchSource.cancel?.()
    setLoading(true)
    setError(undefined)
    setFailed(false)
    try {
      const found = await searchSource.bootstrap()
      if (generationRef.current !== generation) return
      const shown = found.slice(0, maxMenuItems)
      setResults(shown)
      setActiveIndex(shown.length > 0 ? 0 : -1)
      setOpenState(shown.length > 0)
    } catch {
      if (generationRef.current !== generation) return
      setResults([])
      setActiveIndex(-1)
      setError(errorTextValue)
      setFailed(true)
      setOpenState(true)
    } finally {
      if (generationRef.current === generation) setLoading(false)
    }
  }, [errorTextValue, isDisabled, maxMenuItems, searchSource, setOpenState])

  const handleQueryChange = (event: ChangeEvent<HTMLInputElement>) => {
    const nextQuery = event.currentTarget.value
    const queryGeneration = generationRef.current + 1
    invalidatePending()
    setQuery(nextQuery)
    onChangeQuery?.(nextQuery)
    setError(undefined)
    setFailed(false)
    setResults([])
    setActiveIndex(-1)
    setLoading(false)
    if (nextQuery.length === 0) {
      setOpenState(false)
      return
    }
    setOpenState(true)
    if (debounceMs <= 0) {
      void runSearch(nextQuery)
    } else {
      timeoutRef.current = setTimeout(() => {
        timeoutRef.current = undefined
        if (generationRef.current !== queryGeneration) return
        void runSearch(nextQuery)
      }, debounceMs)
    }
  }

  const selectItem = (item: T) => {
    if (isDisabled) return
    invalidatePending()
    onChange(item)
    setQuery("")
    onChangeQuery?.("")
    setResults([])
    setActiveIndex(-1)
    setLoading(false)
    setError(undefined)
    setFailed(false)
    setOpenState(false)
    inputRef.current?.focus()
  }

  const handleKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    onKeyDown?.(event)
    if (event.defaultPrevented || isDisabled) return
    const last = results.length - 1
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault()
        if (!open) {
          if (hasEntriesOnFocus && query.length === 0 && results.length === 0) void runBootstrap()
          else setOpenState(true)
          setActiveIndex(last >= 0 ? 0 : -1)
        } else if (last >= 0) {
          setActiveIndex((current) => current >= last ? 0 : current + 1)
        }
        break
      case "ArrowUp":
        event.preventDefault()
        if (!open) {
          if (hasEntriesOnFocus && query.length === 0 && results.length === 0) void runBootstrap()
          else setOpenState(true)
          setActiveIndex(Math.max(last, 0))
        } else if (last >= 0) {
          setActiveIndex((current) => current <= 0 ? last : current - 1)
        }
        break
      case "Home":
        if (open && last >= 0) {
          event.preventDefault()
          setActiveIndex(0)
        }
        break
      case "End":
        if (open && last >= 0) {
          event.preventDefault()
          setActiveIndex(last)
        }
        break
      case "Enter":
        if (open && activeIndex >= 0 && activeIndex < results.length) {
          event.preventDefault()
          selectItem(results[activeIndex])
        }
        break
      case "Escape":
        if (open) {
          event.preventDefault()
          setOpenState(false)
        } else if (query.length > 0) {
          event.preventDefault()
          clearQuery(false)
        }
        break
      case "Tab":
        if (open) setOpenState(false)
        break
      default:
        break
    }
  }

  useEffect(() => {
    invalidatePending()
    setResults([])
    setActiveIndex(-1)
    setLoading(false)
    setError(undefined)
    setFailed(false)
    setOpenState(false)
    return () => invalidatePending()
  }, [invalidatePending, searchSource, setOpenState])

  useEffect(() => {
    if (!isDisabled) return
    invalidatePending()
    setResults([])
    setActiveIndex(-1)
    setLoading(false)
    setError(undefined)
    setFailed(false)
    setOpenState(false)
  }, [invalidatePending, isDisabled, setOpenState])

  const combinedStatusType = status?.type
  const nativeRequired = required ?? isRequired
  const showEmptyResults = results.length === 0 && !loading && !error && (query.length > 0 || open)
  const emptyResultsId = `${listboxId}-empty`
  const handleFocus = (event: FocusEvent<HTMLInputElement>) => {
    onFocus?.(event)
    if (isDisabled) return
    if (hasEntriesOnFocus && query.length === 0 && results.length === 0 && !open) {
      void runBootstrap()
    } else if (!open && results.length > 0) {
      setOpenState(true)
    }
  }

  const handleFieldBlur = (event: FocusEvent<HTMLElement>) => {
    if (event.currentTarget.contains(event.relatedTarget as Node | null)) return
    setOpenState(false)
  }

  return (
    <section
      className={mergeClasses(fieldClass, className)}
      data-status={status?.type}
      data-state={failed ? "error" : loading ? "loading" : open ? "open" : "closed"}
      aria-busy={loading || undefined}
      onBlur={handleFieldBlur}
    >
      <label className={isLabelHidden ? hiddenLabelClass : labelClass} htmlFor={inputId}>
        {label}
        {nativeRequired ? <small aria-hidden="true"> *</small> : null}
        {isOptional && optionalText ? <small aria-hidden="true"> ({optionalText})</small> : null}
      </label>
      {description ? <p id={descriptionId} className={descriptionClass}>{description}</p> : null}
      {value && query.length === 0 ? <output className="text-sm text-secondary">{value.label}</output> : null}
      <input
        {...domProps}
        ref={setInputRef}
        id={inputId}
        className={inputClasses[size]}
        type="search"
        role="combobox"
        value={query}
        placeholder={placeholderText}
        aria-label={searchText}
        aria-autocomplete="list"
        aria-expanded={open}
        aria-controls={listboxId}
        aria-activedescendant={open && activeIndex >= 0 && activeIndex < results.length ? `${listboxId}-option-${activeIndex}` : undefined}
        aria-describedby={describedBy}
        aria-invalid={failed || combinedStatusType === "error" ? true : domProps["aria-invalid"]}
        aria-required={nativeRequired ? true : domProps["aria-required"]}
        aria-busy={loading || domProps["aria-busy"]}
        aria-disabled={isDisabled ? true : domProps["aria-disabled"]}
        disabled={isDisabled}
        required={nativeRequired}
        data-testid={testId}
        autoComplete="off"
        autoFocus={hasAutoFocus}
        tabIndex={tabIndex}
        onBlur={onBlur}
        onFocus={handleFocus}
        onKeyDown={handleKeyDown}
        onMouseDown={onMouseDown}
        onClick={onClick}
        onChange={handleQueryChange}
      />
      {hasClear && clearText && (query.length > 0 || value !== null) ? (
        <button
          type="button"
          aria-label={clearText}
          disabled={isDisabled}
          onClick={() => {
            if (query.length > 0) clearQuery(true)
            else {
              onChange(null)
              clearQuery(true)
            }
            inputRef.current?.focus()
          }}
        >
          {clearText}
        </button>
      ) : null}
      {htmlName && value !== null ? <input type="hidden" name={htmlName} value={value.id} disabled={isDisabled} /> : null}
      <ul
        id={listboxId}
        className={listboxClass}
        role="listbox"
        aria-label={resultsText}
        aria-describedby={showEmptyResults ? emptyResultsId : undefined}
        hidden={!open}
      >
        {results.map((item, index) => (
          <li
            key={item.id}
            id={`${listboxId}-option-${index}`}
            role="option"
            aria-selected={value?.id === item.id}
            data-highlighted={index === activeIndex || undefined}
            className={optionClass}
            onMouseDown={(event) => event.preventDefault()}
            onMouseEnter={() => setActiveIndex(index)}
            onClick={() => selectItem(item)}
          >
            {renderItem ? renderItem(item) : renderOption ? renderOption(item) : item.element ?? item.label}
          </li>
        ))}
      </ul>
      {showEmptyResults ? <p id={emptyResultsId} role="status" aria-live="polite" className="px-2 py-1 text-sm text-secondary">{noResultsText}</p> : null}
      {isDisabled && disabledMessage ? <p id={disabledMessageId} className={descriptionClass} role="status">{disabledMessage}</p> : null}
      {loading ? <output className="text-xs text-secondary" role="status" aria-live="polite">{loadingTextValue}</output> : null}
      {error ? (
        <p id={errorId} className={statusClasses[statusVariant]} role="alert" aria-live="assertive">
          {error}
        </p>
      ) : null}
      {status?.message ? (
        <p
          id={statusId}
          className={statusClasses[statusVariant]}
          role={combinedStatusType === "error" ? "alert" : "status"}
        >
          {status.message}
        </p>
      ) : null}
    </section>
  )
}

const TypeaheadComponent = forwardRef(TypeaheadImpl)
TypeaheadComponent.displayName = "Typeahead"
export const Typeahead = TypeaheadComponent as unknown as <T extends SearchableItem>(props: TypeaheadProps<T> & { ref?: Ref<HTMLInputElement> }) => ReactElement

export default Typeahead
