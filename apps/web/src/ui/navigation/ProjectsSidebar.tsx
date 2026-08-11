import { useEffect, useRef } from "react"

import { NavigationIcon } from "./icons"
import { ProjectPicker } from "./ProjectPicker"
import { ProjectTree } from "./ProjectTree"
import styles from "./navigation.module.css"
import {
  mergeNavigationLabels,
  type ContextNavigationSection,
  type NavigationLabels,
  type NavigationProject,
  type ProjectPickerStatus,
  type ProjectSurface,
} from "./types"

type ProjectsSidebarBaseProps = {
  readonly projects: readonly NavigationProject[]
  readonly activeProjectSlug?: string
  readonly activeSurface?: ProjectSurface
  readonly activeSection?: ContextNavigationSection
  readonly projectStatus?: ProjectPickerStatus
  readonly onProjectSelect?: (project: NavigationProject) => void
  readonly onSurfaceSelect?: (project: NavigationProject, surface: Exclude<ProjectSurface, "projects">) => void
  readonly onSectionSelect?: (section: ContextNavigationSection) => void
  readonly onProjectRetry?: () => void
  readonly projectSnapshotAvailable?: boolean
  readonly isRefreshing?: boolean
  readonly open?: boolean
  readonly onClose?: () => void
  /** Explicit seam shared with ResourceHeader's `aria-controls`. */
  readonly drawerId?: string
  readonly labels?: Partial<NavigationLabels>
  readonly className?: string
}

type ProjectsSidebarQueryProps =
  | {
      readonly projectQuery: string
      readonly onProjectQueryChange: (query: string) => void
    }
  | {
      readonly projectQuery?: never
      readonly onProjectQueryChange?: never
    }

export type ProjectsSidebarProps = ProjectsSidebarBaseProps & ProjectsSidebarQueryProps

const defaultDrawerId = "projects-sidebar"
const focusableSelector = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "[tabindex]:not([tabindex=\"-1\"])",
].join(",")

function focusableElements(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(focusableSelector)).filter((element) => {
    return !element.hasAttribute("hidden") && element.getAttribute("aria-hidden") !== "true"
  })
}

/**
 * Projects context navigation. The rail owns product-wide destinations; this
 * sidebar owns only Home, Projects and the current project's two surfaces.
 */
export function ProjectsSidebar({
  projects,
  activeProjectSlug,
  activeSurface,
  activeSection,
  projectStatus = "ready",
  projectQuery,
  onProjectQueryChange,
  onProjectSelect,
  onSurfaceSelect,
  onSectionSelect,
  onProjectRetry,
  projectSnapshotAvailable,
  isRefreshing,
  open,
  onClose,
  drawerId: drawerIdOverride,
  labels: labelOverrides,
  className,
}: ProjectsSidebarProps) {
  const labels = mergeNavigationLabels(labelOverrides)
  const drawerId = drawerIdOverride ?? defaultDrawerId
  const headingId = `${drawerId}-heading`
  const isDrawer = open !== undefined
  const isOpen = open !== false
  const sidebarRef = useRef<HTMLElement>(null)
  const returnFocusRef = useRef<HTMLElement | null>(null)
  const wasOpenRef = useRef(false)
  const onCloseRef = useRef(onClose)
  const currentProject = projects.find((project) => project.slug === activeProjectSlug)
  const selectedSection = activeSection ?? (currentProject === undefined ? "projects" : "project")
  const rootClassName = className === undefined ? styles.projectsSidebar : `${styles.projectsSidebar} ${className}`

  useEffect(() => {
    onCloseRef.current = onClose
  }, [onClose])

  useEffect(() => {
    if (!isDrawer || open === undefined) return

    if (open && !wasOpenRef.current) {
      const activeElement = document.activeElement
      returnFocusRef.current = activeElement instanceof HTMLElement ? activeElement : null
      const sidebar = sidebarRef.current
      const firstFocusable = sidebar === null ? null : focusableElements(sidebar)[0]
      firstFocusable?.focus()
    } else if (!open && wasOpenRef.current) {
      returnFocusRef.current?.focus()
      returnFocusRef.current = null
    }
    wasOpenRef.current = open
  }, [isDrawer, open])

  useEffect(() => {
    if (!isDrawer || open !== true) return

    const handleKeyDown = (event: KeyboardEvent): void => {
      const sidebar = sidebarRef.current
      if (sidebar === null) return

      if (event.key === "Escape") {
        event.preventDefault()
        onCloseRef.current?.()
        return
      }

      if (event.key !== "Tab") return
      const focusables = focusableElements(sidebar)
      if (focusables.length === 0) {
        event.preventDefault()
        sidebar.focus()
        return
      }

      const first = focusables[0]
      const last = focusables[focusables.length - 1]
      const activeElement = document.activeElement
      if (!sidebar.contains(activeElement)) {
        event.preventDefault()
        const target = event.shiftKey ? last : first
        target.focus()
      } else if (event.shiftKey && activeElement === first) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && activeElement === last) {
        event.preventDefault()
        first.focus()
      }
    }

    document.addEventListener("keydown", handleKeyDown)
    return () => document.removeEventListener("keydown", handleKeyDown)
  }, [isDrawer, open])

  const handleProjectSelect = (project: NavigationProject) => {
    onProjectSelect?.(project)
  }

  const pickerQueryProps = projectQuery !== undefined && onProjectQueryChange !== undefined
    ? { query: projectQuery, onQueryChange: onProjectQueryChange }
    : {}

  return (
    <>
      {isDrawer && isOpen ? (
        <div
          className={styles.sidebarBackdrop}
          aria-hidden="true"
          role="presentation"
          data-testid="projects-sidebar-backdrop"
          onClick={onClose}
        />
      ) : null}
      <aside
        ref={sidebarRef}
        id={drawerId}
        className={rootClassName}
        role={isDrawer ? "dialog" : undefined}
        aria-label={isDrawer ? undefined : `${labels.projects} context`}
        aria-labelledby={isDrawer ? headingId : undefined}
        aria-modal={isDrawer && open === true ? true : undefined}
        aria-hidden={isDrawer && open === false ? true : undefined}
        inert={isDrawer && open === false ? true : undefined}
        tabIndex={isDrawer && open === false ? -1 : undefined}
        data-testid="projects-sidebar"
        data-open={open === undefined ? undefined : String(open)}
      >
        <div className={styles.sidebarDrawerContent}>
          <header className={styles.sidebarHeader}>
            <h2 id={headingId} className={styles.sidebarTitle}>{labels.projects}</h2>
            {onClose !== undefined ? (
              <button type="button" className={styles.sidebarClose} aria-label={isDrawer ? labels.closeProjectNavigation : labels.close} onClick={onClose} data-testid="projects-sidebar-close">
                <NavigationIcon name="close" size={17} />
              </button>
            ) : null}
          </header>

          <div className={styles.sidebarScrollRegion}>
            <nav className={styles.sidebarNavigation} aria-label={`${labels.projects} navigation`}>
          <button
            type="button"
            className={`${styles.sidebarNavItem} ${selectedSection === "home" ? styles.sidebarNavItemActive : ""}`}
            aria-current={selectedSection === "home" ? "page" : undefined}
            onClick={() => onSectionSelect?.("home")}
            data-testid="projects-sidebar-home"
          >
            <NavigationIcon name="home" size={17} />
            <span>{labels.home}</span>
          </button>
          <button
            type="button"
            className={`${styles.sidebarNavItem} ${selectedSection === "projects" ? styles.sidebarNavItemActive : ""}`}
            aria-current={selectedSection === "projects" ? "page" : undefined}
            onClick={() => onSectionSelect?.("projects")}
            data-testid="projects-sidebar-projects"
          >
            <NavigationIcon name="folder" size={17} />
            <span>{labels.projects}</span>
          </button>
            </nav>

            <div className={styles.sidebarDivider} />

            <ProjectPicker
              projects={projects}
              activeProjectSlug={activeProjectSlug}
              status={projectStatus}
              {...pickerQueryProps}
              onSelect={handleProjectSelect}
              onRetry={onProjectRetry}
              hasSnapshot={projectSnapshotAvailable}
              isRefreshing={isRefreshing}
              labels={labels}
            />

            {currentProject !== undefined && selectedSection === "project" ? (
              <ProjectTree
                project={currentProject}
                activeSurface={activeSurface}
                onProjectSelect={handleProjectSelect}
                onSurfaceSelect={onSurfaceSelect}
                labels={labels}
              />
            ) : null}
          </div>
        </div>
      </aside>
    </>
  )
}

export default ProjectsSidebar
