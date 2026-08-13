import { useEffect, useLayoutEffect, useRef, type MouseEvent } from "react"

import { NavigationIcon } from "./icons"
import { ProjectPicker } from "./ProjectPicker"
import { ProjectTree } from "./ProjectTree"
import { SidebarResizeHandle } from "./SidebarResizeHandle"
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
  readonly projectsHref?: string
  readonly basePath?: string
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
  readonly sidebarWidthStep?: number
  readonly onSidebarWidthStepChange?: (step: number) => void
  readonly onSidebarWidthReset?: () => void
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
 * sidebar owns Projects and the current project's two surfaces.
 */
export function ProjectsSidebar({
  projects,
  activeProjectSlug,
  activeSurface,
  activeSection,
  projectStatus = "ready",
  projectsHref,
  basePath,
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
  sidebarWidthStep,
  onSidebarWidthStepChange,
  onSidebarWidthReset,
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

  useLayoutEffect(() => {
    if (!isDrawer || open === undefined) return

    const focusFirstElement = () => {
      const sidebar = sidebarRef.current
      const firstFocusable = sidebar === null ? null : focusableElements(sidebar)[0]
      if (firstFocusable !== null) firstFocusable.focus()
      else sidebar?.focus()
    }
    const focusIfOpen = () => {
      const sidebar = sidebarRef.current
      if (sidebar?.getAttribute("data-open") !== "true" || sidebar.contains(document.activeElement)) return
      focusFirstElement()
    }

    if (open && !wasOpenRef.current) {
      const activeElement = document.activeElement
      returnFocusRef.current = activeElement instanceof HTMLElement ? activeElement : null
    }

    if (open) {
      // Focus on every effect setup while open. React StrictMode deliberately
      // replays layout effects; repeating this idempotent focus keeps an actual
      // closed → open transition inside the dialog after that replay.
      focusIfOpen()
      if (typeof window !== "undefined") {
        window.requestAnimationFrame(() => {
          focusIfOpen()
          window.requestAnimationFrame(focusIfOpen)
        })
      }
    } else if (wasOpenRef.current) {
      const returnFocus = returnFocusRef.current
      if (returnFocus?.isConnected && typeof window !== "undefined") {
        window.requestAnimationFrame(() => {
          if (sidebarRef.current?.getAttribute("data-open") === "false") returnFocus.focus()
          window.requestAnimationFrame(() => {
            if (sidebarRef.current?.getAttribute("data-open") === "false") returnFocus.focus()
          })
        })
      } else if (returnFocus?.isConnected) returnFocus.focus()
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
          {projectsHref === undefined ? (
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
          ) : (
            <a
              className={`${styles.sidebarNavItem} ${selectedSection === "projects" ? styles.sidebarNavItemActive : ""}`}
              aria-current={selectedSection === "projects" ? "page" : undefined}
              href={projectsHref}
              onClick={(event: MouseEvent<HTMLAnchorElement>) => {
                if (onSectionSelect === undefined || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
                event.preventDefault()
                onSectionSelect("projects")
              }}
              data-testid="projects-sidebar-projects"
            >
              <NavigationIcon name="folder" size={17} />
              <span>{labels.projects}</span>
            </a>
          )}
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
                basePath={basePath}
                labels={labels}
              />
            ) : null}
          </div>
          {!isDrawer && sidebarWidthStep !== undefined && onSidebarWidthStepChange !== undefined && onSidebarWidthReset !== undefined ? (
            <SidebarResizeHandle step={sidebarWidthStep} onStepChange={onSidebarWidthStepChange} onReset={onSidebarWidthReset} />
          ) : null}
        </div>
      </aside>
    </>
  )
}

export default ProjectsSidebar
