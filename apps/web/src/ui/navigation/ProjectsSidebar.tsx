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

export type ProjectsSidebarProps = {
  readonly projects: readonly NavigationProject[]
  readonly activeProjectSlug?: string
  readonly activeSurface?: ProjectSurface
  readonly activeSection?: ContextNavigationSection
  readonly projectStatus?: ProjectPickerStatus
  readonly projectQuery?: string
  readonly onProjectQueryChange?: (query: string) => void
  readonly onProjectSelect?: (project: NavigationProject) => void
  readonly onSurfaceSelect?: (project: NavigationProject, surface: Exclude<ProjectSurface, "projects">) => void
  readonly onSectionSelect?: (section: ContextNavigationSection) => void
  readonly onProjectRetry?: () => void
  readonly open?: boolean
  readonly onClose?: () => void
  readonly labels?: Partial<NavigationLabels>
  readonly className?: string
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
  open,
  onClose,
  labels: labelOverrides,
  className,
}: ProjectsSidebarProps) {
  const labels = mergeNavigationLabels(labelOverrides)
  const currentProject = projects.find((project) => project.slug === activeProjectSlug)
  const selectedSection = activeSection ?? (currentProject === undefined ? "projects" : "project")
  const rootClassName = className === undefined ? styles.projectsSidebar : `${styles.projectsSidebar} ${className}`

  const handleProjectSelect = (project: NavigationProject) => {
    onProjectSelect?.(project)
  }

  return (
    <aside
      className={rootClassName}
      aria-label={`${labels.projects} context`}
      aria-hidden={open === false ? true : undefined}
      data-testid="projects-sidebar"
      data-open={open === undefined ? undefined : String(open)}
    >
      <header className={styles.sidebarHeader}>
        <h2 className={styles.sidebarTitle}>{labels.projects}</h2>
        {onClose !== undefined ? (
          <button type="button" className={styles.sidebarClose} aria-label={labels.close} onClick={onClose}>
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
          query={projectQuery}
          onQueryChange={onProjectQueryChange}
          onSelect={handleProjectSelect}
          onRetry={onProjectRetry}
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
    </aside>
  )
}

export default ProjectsSidebar
