import {
  forwardRef,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type ChangeEvent,
  type FocusEvent,
  type KeyboardEvent,
  type ReactNode,
  type Ref,
} from "react"

import {
  descriptionClass,
  fieldClass,
  flattenOptions,
  groupClass,
  inputClasses,
  hiddenLabelClass,
  joinIds,
  labelClass,
  listboxClass,
  mergeClasses,
  optionClass,
  optionGroups,
  srOnlyClass,
  selectorTriggerClasses,
  statusClasses,
  type SafeDomProps,
  type SelectableOption,
  type SelectorOption,
  type SelectorOptionData,
  type SelectorStatus,
} from "./shared"

export type { SelectorOptionData, SelectorOptionType, SelectorOption, SelectorStatus } from "./shared"

export type MultiSelectorOptionData = SelectorOptionData
export type MultiSelectorOptionType = SelectorOption
export type MultiSelectorStatus = SelectorStatus

type MultiSelectorPropsBase = SafeDomProps & {
  readonly label: string
  readonly description?: string
  readonly status?: SelectorStatus
  readonly statusVariant?: "attached" | "detached"
  readonly htmlName?: string
  readonly options: readonly SelectorOption[]
  readonly value: readonly string[]
  readonly onChange: (value: string[]) => void
  readonly renderOption?: (option: SelectableOption) => ReactNode
  readonly placeholder: string
  readonly isLabelHidden?: boolean
  readonly isDisabled?: boolean
  readonly isLoading?: boolean
  readonly loadingText: string
  readonly size?: "sm" | "md" | "lg"
  readonly variant?: "input" | "ghost"
  readonly maxBadges?: number
  readonly noOptionsText: string
  readonly isDefaultOpen?: boolean
  readonly onOpenChange?: (open: boolean) => void
  readonly required?: boolean
  readonly "data-testid"?: string
}

type MultiSelectorRequirementProps =
  | { readonly isRequired: true; readonly isOptional?: false; readonly optionalLabel?: never }
  | { readonly isRequired?: false; readonly isOptional: true; readonly optionalLabel: string }
  | { readonly isRequired?: false; readonly isOptional?: false; readonly optionalLabel?: never }

type MultiSelectorClearProps =
  | { readonly hasClear: true; readonly clearLabel: string }
  | { readonly hasClear?: false; readonly clearLabel?: never }

type MultiSelectorSearchProps =
  | { readonly hasSearch: true; readonly searchLabel: string; readonly searchPlaceholder: string }
  | { readonly hasSearch?: false; readonly searchLabel?: never; readonly searchPlaceholder?: never }

type MultiSelectorSelectAllProps =
  | {
      readonly hasSelectAll: true
      readonly selectAllLabel: string
      readonly selectAllStateLabel?: (state: "all" | "some" | "none") => string
    }
  | { readonly hasSelectAll?: false; readonly selectAllLabel?: never; readonly selectAllStateLabel?: never }

type MultiSelectorDisplayProps =
  | { readonly triggerDisplay?: "count"; readonly selectedText: (count: number) => string }
  | { readonly triggerDisplay: "labels" | "badges"; readonly selectedText?: (count: number) => string }

export type MultiSelectorProps = MultiSelectorPropsBase & MultiSelectorRequirementProps & MultiSelectorClearProps & MultiSelectorSearchProps & MultiSelectorSelectAllProps & MultiSelectorDisplayProps

function selectedLabels(options: readonly SelectableOption[], values: readonly string[]): readonly string[] {
  const labels = new Map(options.map((option) => [option.value, option.label]))
  return values.map((value) => labels.get(value) ?? value)
}

function MultiSelectorImpl(
  {
    label,
    description,
    status,
    statusVariant = "attached",
    htmlName,
    options,
    value,
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
    triggerDisplay = "count",
    selectedText,
    maxBadges = 3,
    hasClear = false,
    clearLabel,
    hasSelectAll = false,
    selectAllLabel,
    selectAllStateLabel,
    hasSearch = false,
    searchLabel,
    searchPlaceholder,
    noOptionsText,
    isDefaultOpen = false,
    onOpenChange,
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
  }: MultiSelectorProps,
  forwardedRef: Ref<HTMLButtonElement>,
) {
  const generatedId = useId().replaceAll(":", "")
  const triggerId = id ?? `multi-selector-${generatedId}`
  const listboxId = `${triggerId}-listbox`
  const descriptionId = description ? `${triggerId}-description` : undefined
  const statusId = status?.message ? `${triggerId}-status` : undefined
  const searchId = `${triggerId}-search`
  const triggerRef = useRef<HTMLButtonElement>(null)
  const searchRef = useRef<HTMLInputElement>(null)
  const [open, setOpen] = useState(isDefaultOpen)
  const [activeIndex, setActiveIndex] = useState(-1)
  const [searchQuery, setSearchQuery] = useState("")
  const openRef = useRef(open)
  const onOpenChangeRef = useRef(onOpenChange)
  openRef.current = open
  onOpenChangeRef.current = onOpenChange
  const allOptions = useMemo(() => flattenOptions(options), [options])
  const enabledOptions = useMemo(() => allOptions.filter((option) => !option.disabled), [allOptions])
  const selectAllValue = useMemo(() => {
    const values = new Set(allOptions.map((option) => option.value))
    let candidate = "__astryx_select_all__"
    while (values.has(candidate)) candidate = `${candidate}_`
    return candidate
  }, [allOptions])
  const searchFilter = searchQuery.trim().toLocaleLowerCase()
  const hasSearchFilter = searchFilter.length > 0
  const visibleOptions = useMemo(() => {
    if (!hasSearchFilter) return allOptions
    return allOptions.filter((option) => option.label.toLocaleLowerCase().includes(searchFilter))
  }, [allOptions, hasSearchFilter, searchFilter])
  const clearText = clearLabel
  const selectAllText = selectAllLabel
  const searchText = searchLabel
  const selectAllOption = useMemo<SelectableOption>(() => ({ value: selectAllValue, label: selectAllText ?? "" }), [selectAllValue, selectAllText])
  const showSelectAll = hasSelectAll && enabledOptions.length > 0
  const activeOptions = useMemo(
    () => showSelectAll && !hasSearchFilter ? [selectAllOption, ...visibleOptions] : visibleOptions,
    [hasSearchFilter, selectAllOption, showSelectAll, visibleOptions],
  )
  const selectedSet = useMemo(() => new Set(value), [value])
  const allSelected = enabledOptions.length > 0 && enabledOptions.every((option) => selectedSet.has(option.value))
  const selectAllState = allSelected
    ? "all"
    : enabledOptions.some((option) => selectedSet.has(option.value)) ? "some" : "none"
  const selectAllAnnouncement = selectAllStateLabel?.(selectAllState)
  const selected = useMemo(() => selectedLabels(allOptions, value), [allOptions, value])
  const describedBy = joinIds(domProps["aria-describedby"], descriptionId, statusId)
  const disabled = isDisabled || isLoading
  const nativeRequired = required ?? isRequired
  const focusableIndices = useMemo(
    () => activeOptions.flatMap((option, index) => option.disabled ? [] : [index]),
    [activeOptions],
  )
  const firstFocusableIndex = focusableIndices[0] ?? -1
  const lastFocusableIndex = focusableIndices[focusableIndices.length - 1] ?? -1
  const activeOption = activeOptions[activeIndex]
  const emptyOptions = open && activeOptions.length === (showSelectAll && !hasSearchFilter ? 1 : 0)
  const emptyOptionsId = `${listboxId}-empty`

  useEffect(() => {
    setActiveIndex((current) => focusableIndices.includes(current) ? current : firstFocusableIndex)
  }, [firstFocusableIndex, focusableIndices])

  useEffect(() => {
    if (open && hasSearch) {
      searchRef.current?.focus()
    }
  }, [hasSearch, open])

  useEffect(() => {
    if (!disabled) return
    if (openRef.current) onOpenChangeRef.current?.(false)
    openRef.current = false
    setOpen(false)
    setActiveIndex(-1)
    setSearchQuery("")
  }, [disabled])

  const setOpenState = (next: boolean, restoreFocus = true, clearQuery = true) => {
    if (disabled || next === openRef.current) return
    openRef.current = next
    setOpen(next)
    onOpenChangeRef.current?.(next)
    if (next) {
      setActiveIndex(firstFocusableIndex)
    } else {
      if (clearQuery) setSearchQuery("")
      if (restoreFocus) triggerRef.current?.focus()
    }
  }

  const handleFieldBlur = (event: FocusEvent<HTMLElement>) => {
    if (event.currentTarget.contains(event.relatedTarget as Node | null)) return
    setOpenState(false, false)
  }

  const toggleOption = (option: SelectableOption) => {
    if (disabled || option.disabled) return
    if (option.value === selectAllValue) {
      onChange(allSelected ? value.filter((item) => !enabledOptions.some((entry) => entry.value === item)) : enabledOptions.map((entry) => entry.value))
      return
    }
    const next = selectedSet.has(option.value)
      ? value.filter((item) => item !== option.value)
      : [...value, option.value]
    onChange(next)
  }

  const handleKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    onKeyDown?.(event)
    if (event.defaultPrevented || disabled) return
    const isSearchInput = event.currentTarget === searchRef.current
    if (isSearchInput && (event.key === " " || event.key === "Home" || event.key === "End")) return
    const moveActive = (direction: 1 | -1) => {
      if (focusableIndices.length === 0) return
      const currentPosition = focusableIndices.indexOf(activeIndex)
      const nextPosition = currentPosition < 0
        ? direction > 0 ? 0 : focusableIndices.length - 1
        : (currentPosition + direction + focusableIndices.length) % focusableIndices.length
      setActiveIndex(focusableIndices[nextPosition] ?? firstFocusableIndex)
    }
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault()
        if (!open) setOpenState(true)
        else moveActive(1)
        break
      case "ArrowUp":
        event.preventDefault()
        if (!open) {
          setOpenState(true)
          setActiveIndex(lastFocusableIndex)
        } else {
          moveActive(-1)
        }
        break
      case "Home":
        if (open && firstFocusableIndex >= 0) {
          event.preventDefault()
          setActiveIndex(firstFocusableIndex)
        }
        break
      case "End":
        if (open && lastFocusableIndex >= 0) {
          event.preventDefault()
          setActiveIndex(lastFocusableIndex)
        }
        break
      case "Enter":
      case " ":
        event.preventDefault()
        if (!open) setOpenState(true)
        else if (activeOption) toggleOption(activeOption)
        break
      case "Escape":
        if (open) {
          event.preventDefault()
          setOpenState(false, true, false)
        } else if (hasSearch && searchQuery.length > 0) {
          event.preventDefault()
          setSearchQuery("")
          setActiveIndex(-1)
        }
        break
      case "Tab":
        if (open) setOpenState(false, false)
        break
      default:
        break
    }
  }

  const handleSearchChange = (event: ChangeEvent<HTMLInputElement>) => {
    setSearchQuery(event.currentTarget.value)
    setActiveIndex(-1)
  }

  const summary = selected.length === 0
    ? placeholder
    : triggerDisplay === "count"
      ? selectedText?.(selected.length) ?? ""
      : triggerDisplay === "badges"
        ? selected.slice(0, maxBadges).join(", ") + (selected.length > maxBadges ? ` +${selected.length - maxBadges}` : "")
        : selected.join(", ")

  const renderOptionItem = (option: SelectableOption, index: number) => {
    const selectedOption = option.value === selectAllValue ? allSelected : selectedSet.has(option.value)
    const isActive = index === activeIndex
    return (
      <li
        key={option.value}
        id={`${listboxId}-option-${index}`}
        role="option"
        aria-selected={selectedOption}
        aria-label={option.value === selectAllValue && selectAllAnnouncement ? `${selectAllText}, ${selectAllAnnouncement}` : undefined}
        aria-disabled={option.disabled || undefined}
        data-partial={option.value === selectAllValue && selectAllState === "some" ? "true" : undefined}
        data-highlighted={isActive || undefined}
        data-selected={selectedOption || undefined}
        data-disabled={option.disabled || undefined}
        className={optionClass}
        onMouseDown={(event) => event.preventDefault()}
        onMouseEnter={() => { if (!option.disabled) setActiveIndex(index) }}
        onClick={() => toggleOption(option)}
      >
        {option.value === selectAllValue ? selectAllText : renderOption ? renderOption(option) : option.label}
      </li>
    )
  }

  const renderedGroups = optionGroups(options).map((group, groupIndex) => {
    const filtered = group.options.filter((option) => visibleOptions.some((visible) => visible.value === option.value))
    if (filtered.length === 0) return null
    return group.title === undefined ? filtered.map((option) => renderOptionItem(option, activeOptions.findIndex((entry) => entry.value === option.value))) : (
      <li key={`group-${groupIndex}`} role="group" aria-label={group.title} className={groupClass}>
        <strong className="px-2 py-1 text-xs font-medium text-secondary">{group.title}</strong>
        <ul role="presentation">
          {filtered.map((option) => renderOptionItem(option, activeOptions.findIndex((entry) => entry.value === option.value)))}
        </ul>
      </li>
    )
  })

  const setTriggerRef = (node: HTMLButtonElement | null) => {
    triggerRef.current = node
    if (typeof forwardedRef === "function") forwardedRef(node)
    else if (forwardedRef) forwardedRef.current = node
  }

  return (
    <section
      className={mergeClasses(fieldClass, className)}
      aria-busy={isLoading || undefined}
      data-status={status?.type}
      onBlur={handleFieldBlur}
    >
      <label className={isLabelHidden ? hiddenLabelClass : labelClass} htmlFor={triggerId}>
        {label}
        {nativeRequired ? <small aria-hidden="true"> *</small> : null}
        {isOptional ? <small aria-hidden="true"> ({optionalLabel})</small> : null}
      </label>
      {description ? <p id={descriptionId} className={descriptionClass}>{description}</p> : null}
      <button
        {...domProps}
        ref={setTriggerRef}
        id={triggerId}
        type="button"
        className={selectorTriggerClasses[variant][size]}
        role={hasSearch ? undefined : "combobox"}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listboxId}
        aria-activedescendant={!hasSearch && open && activeOption && !activeOption.disabled ? `${listboxId}-option-${activeIndex}` : undefined}
        aria-describedby={describedBy}
        aria-required={nativeRequired ? true : domProps["aria-required"]}
        aria-invalid={status?.type === "error" ? true : domProps["aria-invalid"]}
        aria-busy={isLoading || domProps["aria-busy"]}
        disabled={disabled}
        tabIndex={tabIndex}
        onBlur={onBlur}
        onFocus={onFocus}
        onKeyDown={handleKeyDown}
        onMouseDown={onMouseDown}
        onClick={(event) => {
          onClick?.(event)
          if (!event.defaultPrevented) setOpenState(!open)
        }}
        data-testid={testId}
      >
        {summary}
      </button>
      {hasClear && value.length > 0 && clearText ? (
        <button type="button" aria-label={clearText} disabled={disabled} onClick={() => onChange([])}>
          {clearText}
        </button>
      ) : null}
      {htmlName ? value.map((selectedValue, index) => (
        <input
          key={`${selectedValue}-${index}`}
          type="hidden"
          name={htmlName}
          value={selectedValue}
          disabled={disabled}
        />
      )) : null}
      {hasSearch && open && searchText ? (
        <label className={srOnlyClass} htmlFor={searchId}>{searchText}</label>
      ) : null}
      {hasSearch && open ? (
        <input
          ref={searchRef}
          id={searchId}
          className={inputClasses[size]}
          type="search"
          value={searchQuery}
          placeholder={searchPlaceholder}
          role="combobox"
          aria-label={searchText}
          aria-labelledby={searchText ? undefined : triggerId}
          aria-autocomplete="list"
          aria-expanded={open}
          aria-controls={listboxId}
          aria-activedescendant={activeOption && !activeOption.disabled ? `${listboxId}-option-${activeIndex}` : undefined}
          aria-describedby={describedBy}
          aria-invalid={status?.type === "error" ? true : domProps["aria-invalid"]}
          aria-required={nativeRequired ? true : domProps["aria-required"]}
          aria-busy={isLoading || domProps["aria-busy"]}
          aria-disabled={disabled ? true : domProps["aria-disabled"]}
          autoComplete="off"
          disabled={disabled}
          required={nativeRequired}
          data-testid={testId ? `${testId}-search` : undefined}
          onChange={handleSearchChange}
          onKeyDown={handleKeyDown}
        />
      ) : null}
      <ul
        id={listboxId}
        className={listboxClass}
        role="listbox"
        aria-labelledby={triggerId}
        aria-describedby={emptyOptions ? emptyOptionsId : undefined}
        aria-multiselectable="true"
        hidden={!open}
      >
        {showSelectAll && !hasSearchFilter ? renderOptionItem(selectAllOption, 0) : null}
        {renderedGroups}
      </ul>
      {emptyOptions ? <p id={emptyOptionsId} role="status" aria-live="polite" className="px-2 py-1 text-sm text-secondary">{noOptionsText}</p> : null}
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

export const MultiSelector = forwardRef<HTMLButtonElement, MultiSelectorProps>(MultiSelectorImpl)
MultiSelector.displayName = "MultiSelector"

export default MultiSelector
