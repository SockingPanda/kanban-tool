import { useEffect, useId, useMemo, useState, type ChangeEvent, type KeyboardEvent, type Ref } from "react"

import { NavigationIcon } from "./icons"
import styles from "./navigation.module.css"
import { defaultNavigationLabels, mergeNavigationLabels, type NavigationLabels, type NavigationProject, type ProjectPickerStatus } from "./types"

export type ProjectPickerSearchProps = {
  readonly query: string
  readonly onQueryChange: (query: string) => void
  readonly placeholder?: string
  readonly ariaLabel?: string
  readonly disabled?: boolean
  readonly inputId?: string
  readonly listboxId?: string
  readonly activeDescendant?: string
  readonly expanded?: boolean
  readonly clearLabel?: string
  readonly onKeyDown?: (event: KeyboardEvent<HTMLInputElement>) => void
  readonly onClear?: () => void
  readonly inputRef?: Ref<HTMLInputElement>
}

/** Search input kept separate so a shell can place it outside the result list. */
export function ProjectPickerSearch({
  query,
  onQueryChange,
  placeholder = defaultNavigationLabels.projectSearchPlaceholder,
  ariaLabel = defaultNavigationLabels.projectSearch,
  disabled = false,
  inputId,
  listboxId,
  activeDescendant,
  expanded = false,
  clearLabel = "Clear search",
  onKeyDown,
  onClear,
  inputRef,
}: ProjectPickerSearchProps) {
  const handleChange = (event: ChangeEvent<HTMLInputElement>) => onQueryChange(event.currentTarget.value)

  return (
    <div className={styles.projectPickerSearch} data-testid="project-picker-search">
      <NavigationIcon name="search" size={16} />
      <input
        ref={inputRef}
        id={inputId}
        className={styles.projectPickerSearchInput}
        type="search"
        value={query}
        placeholder={placeholder}
        aria-label={ariaLabel}
        role="combobox"
        aria-autocomplete="list"
        aria-expanded={expanded}
        aria-controls={listboxId === undefined ? undefined : listboxId}
        aria-activedescendant={activeDescendant === undefined ? undefined : activeDescendant}
        autoComplete="off"
        disabled={disabled}
        data-project-picker-input="true"
        onChange={handleChange}
        onKeyDown={onKeyDown}
      />
      {query.length > 0 && onClear !== undefined ? (
        <button
          type="button"
          className={styles.projectPickerClear}
          aria-label={clearLabel}
          onClick={onClear}
          disabled={disabled}
        >
          <NavigationIcon name="close" size={13} />
        </button>
      ) : null}
    </div>
  )
}

type ProjectPickerPropsBase = {
  readonly projects: readonly NavigationProject[]
  readonly activeProjectSlug?: string
  readonly status?: ProjectPickerStatus
  readonly onSelect?: (project: NavigationProject) => void
  readonly onRetry?: () => void
  /** True when `projects` is a cached snapshot, including a valid empty snapshot. */
  readonly hasSnapshot?: boolean
  /** Separates an in-flight refresh from a blocking first load. */
  readonly isRefreshing?: boolean
  readonly labels?: Partial<NavigationLabels>
  readonly className?: string
}

export type ProjectPickerQueryProps =
  | {
      readonly query: string
      readonly onQueryChange: (query: string) => void
    }
  | {
      readonly query?: never
      readonly onQueryChange?: never
    }

export type ProjectPickerProps = ProjectPickerPropsBase & ProjectPickerQueryProps

function projectMatches(project: NavigationProject, query: string): boolean {
  const normalizedQuery = query.trim().toLocaleLowerCase()
  if (normalizedQuery.length === 0) return true
  return [project.name, project.slug, project.description ?? ""].some((value) => value.toLocaleLowerCase().includes(normalizedQuery))
}

function statusMessage(status: ProjectPickerStatus, labels: NavigationLabels, hasQuery: boolean): string {
  if (status === "loading") return labels.projectLoading
  if (status === "offline") return labels.projectOffline
  if (status === "error") return labels.projectError
  if (status === "stale") return labels.projectStale
  if (status === "recovering") return labels.projectRecovering
  return hasQuery ? labels.projectSearchEmpty : labels.projectsEmpty
}

/**
 * A controlled-friendly, keyboard-first project switcher. It is intentionally
 * read-only: selecting an option only emits the canonical-shaped project.
 */
export function ProjectPicker({
  projects,
  activeProjectSlug,
  status = "ready",
  query,
  onQueryChange,
  onSelect,
  onRetry,
  hasSnapshot: hasSnapshotOverride,
  isRefreshing = false,
  labels: labelOverrides,
  className,
}: ProjectPickerProps) {
  const labels = mergeNavigationLabels(labelOverrides)
  const [internalQuery, setInternalQuery] = useState("")
  const [activeIndex, setActiveIndex] = useState(0)
  const generatedId = useId().replaceAll(":", "")
  const listboxId = `project-picker-list-${generatedId}`
  const inputId = `project-picker-input-${generatedId}`
  const currentQuery = query ?? internalQuery
  const setQuery = onQueryChange ?? setInternalQuery
  const filteredProjects = useMemo(
    () => projects.filter((project) => projectMatches(project, currentQuery)),
    [currentQuery, projects],
  )
  const activeOption = filteredProjects[activeIndex]
  const hasSnapshot = hasSnapshotOverride ?? projects.length > 0
  const disabled = status === "loading" && !hasSnapshot
  const boundaryStatus = isRefreshing && status === "ready" ? "recovering" : status
  const hasOptions = !disabled && filteredProjects.length > 0
  const activeDescendant = hasOptions && activeOption !== undefined ? `${listboxId}-${activeOption.id}` : undefined

  useEffect(() => {
    setActiveIndex((index) => Math.min(index, Math.max(filteredProjects.length - 1, 0)))
  }, [filteredProjects.length])

  const handleQueryChange = (nextQuery: string) => {
    setActiveIndex(0)
    setQuery(nextQuery)
  }

  const handleSelect = (project: NavigationProject) => {
    onSelect?.(project)
  }

  const handleSearchKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (disabled || filteredProjects.length === 0) {
      if (event.key === "Escape" && currentQuery.length > 0) {
        event.preventDefault()
        handleQueryChange("")
      }
      return
    }

    if (event.key === "ArrowDown") {
      event.preventDefault()
      setActiveIndex((index) => (index + 1) % filteredProjects.length)
    } else if (event.key === "ArrowUp") {
      event.preventDefault()
      setActiveIndex((index) => (index - 1 + filteredProjects.length) % filteredProjects.length)
    } else if (event.key === "Enter" && activeOption !== undefined) {
      event.preventDefault()
      handleSelect(activeOption)
    } else if (event.key === "Escape" && currentQuery.length > 0) {
      event.preventDefault()
      handleQueryChange("")
    }
  }

  const rootClassName = className === undefined ? styles.projectPicker : `${styles.projectPicker} ${className}`
  const showBoundary = boundaryStatus !== "ready" || filteredProjects.length === 0
  const boundaryIsEmpty = boundaryStatus === "ready" && filteredProjects.length === 0
  const hasRetry = onRetry !== undefined && boundaryStatus !== "ready" && boundaryStatus !== "loading"

  return (
    <section
      className={rootClassName}
      aria-label={labels.projectPicker}
      aria-busy={isRefreshing || boundaryStatus === "loading" || boundaryStatus === "recovering" ? true : undefined}
      data-testid="project-picker"
      data-status={boundaryStatus}
      data-has-snapshot={hasSnapshot ? "true" : "false"}
    >
      <ProjectPickerSearch
        query={currentQuery}
        onQueryChange={handleQueryChange}
        placeholder={labels.projectSearchPlaceholder}
        ariaLabel={labels.projectSearch}
        clearLabel={labels.clearSearch}
        disabled={disabled}
        inputId={inputId}
        listboxId={hasOptions ? listboxId : undefined}
        activeDescendant={activeDescendant}
        expanded={hasOptions}
        onKeyDown={handleSearchKeyDown}
        onClear={() => handleQueryChange("")}
      />

      {showBoundary ? (
        <div
          className={`${styles.projectPickerBoundary} ${boundaryStatus === "error" ? styles.projectPickerBoundaryError : ""}`}
          role={boundaryStatus === "error" || boundaryStatus === "offline" ? "alert" : "status"}
          aria-live="polite"
          data-testid={`project-picker-${boundaryIsEmpty ? "empty" : boundaryStatus}`}
        >
          <span>{statusMessage(boundaryStatus, labels, currentQuery.trim().length > 0)}</span>
          {hasRetry ? (
            <button type="button" className={styles.projectPickerRetry} onClick={onRetry}>
              {labels.retry}
            </button>
          ) : null}
        </div>
      ) : null}

      {hasOptions ? (
        <ul
          id={listboxId}
          className={styles.projectPickerList}
          role="listbox"
          aria-label={labels.projectPicker}
          data-testid="project-picker-options"
        >
          {filteredProjects.map((project, index) => {
            const selected = project.slug === activeProjectSlug
            const highlighted = index === activeIndex
            return (
              <li
                id={`${listboxId}-${project.id}`}
                key={project.id}
                role="option"
                aria-selected={selected}
                className={`${styles.projectPickerOption} ${highlighted ? styles.projectPickerOptionHighlighted : ""}`}
                data-active={highlighted ? "true" : undefined}
                onMouseEnter={() => setActiveIndex(index)}
                onPointerDown={(event) => event.preventDefault()}
                onClick={() => handleSelect(project)}
              >
                <NavigationIcon name="folder" size={17} />
                <span className={styles.projectPickerOptionCopy}>
                  <span className={styles.projectPickerOptionName}>
                    {project.name}
                    {project.archivedAt !== null ? <span className={styles.projectPickerOptionArchive}> · {labels.archived}</span> : null}
                  </span>
                  <span className={styles.projectPickerOptionMeta}>{project.slug}</span>
                </span>
              </li>
            )
          })}
        </ul>
      ) : null}
    </section>
  )
}

export default ProjectPicker
