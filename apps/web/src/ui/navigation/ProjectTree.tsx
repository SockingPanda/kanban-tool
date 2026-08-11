import { useState, type KeyboardEvent } from "react"

import { NavigationIcon } from "./icons"
import styles from "./navigation.module.css"
import { mergeNavigationLabels, type NavigationLabels, type NavigationProject, type ProjectSurface } from "./types"

export type ProjectTreeProps = {
  readonly project: NavigationProject
  readonly activeSurface?: ProjectSurface
  readonly expanded?: boolean
  readonly defaultExpanded?: boolean
  readonly onExpandedChange?: (expanded: boolean) => void
  readonly onProjectSelect?: (project: NavigationProject) => void
  readonly onSurfaceSelect?: (project: NavigationProject, surface: Exclude<ProjectSurface, "projects">) => void
  readonly labels?: Partial<NavigationLabels>
  readonly className?: string
}

/** The current project branch intentionally contains only Overview and Tasks. */
export function ProjectTree({
  project,
  activeSurface,
  expanded,
  defaultExpanded = true,
  onExpandedChange,
  onProjectSelect,
  onSurfaceSelect,
  labels: labelOverrides,
  className,
}: ProjectTreeProps) {
  const labels = mergeNavigationLabels(labelOverrides)
  const [internalExpanded, setInternalExpanded] = useState(defaultExpanded)
  const isExpanded = expanded ?? internalExpanded
  const childrenId = `project-tree-children-${project.id}`
  const rootClassName = className === undefined ? styles.projectTree : `${styles.projectTree} ${className}`

  const setExpanded = (nextExpanded: boolean) => {
    if (expanded === undefined) setInternalExpanded(nextExpanded)
    onExpandedChange?.(nextExpanded)
  }

  const handleToggleKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (event.key === "ArrowRight" && !isExpanded) {
      event.preventDefault()
      setExpanded(true)
    } else if (event.key === "ArrowLeft" && isExpanded) {
      event.preventDefault()
      setExpanded(false)
    }
  }

  const children: readonly { surface: Exclude<ProjectSurface, "projects">; label: string; icon: "grid" | "list" }[] = [
    { surface: "overview", label: labels.overview, icon: "grid" },
    { surface: "tasks", label: labels.tasks, icon: "list" },
  ]

  return (
    <nav className={rootClassName} aria-label={`${project.name} navigation`} data-testid="project-tree">
      <h3 className={styles.projectTreeHeading}>
        <span>{project.name}</span>
        {project.archivedAt !== null ? <span aria-label="Archived">· archived</span> : null}
      </h3>
      <ul className={styles.projectTreeList} aria-label={`${project.name} project sections`}>
        <li>
          <div className={styles.projectTreeProjectRow}>
            <button
              type="button"
              className={styles.projectTreeToggle}
              aria-label={isExpanded ? labels.collapse : labels.expand}
              aria-controls={isExpanded ? childrenId : undefined}
              aria-expanded={isExpanded}
              onClick={() => setExpanded(!isExpanded)}
              onKeyDown={handleToggleKeyDown}
              data-testid="project-tree-toggle"
            >
              <NavigationIcon name={isExpanded ? "chevron-down" : "chevron-right"} size={16} />
            </button>
            <button
              type="button"
              className={styles.projectTreeProjectButton}
              onClick={() => onProjectSelect?.(project)}
              data-testid="project-tree-project"
            >
              <NavigationIcon name="folder" size={17} />
              <span className={styles.projectTreeProjectName}>{project.name}</span>
            </button>
          </div>
          {isExpanded ? (
            <ul id={childrenId} className={styles.projectTreeChildren}>
              {children.map((child) => {
                const isActive = activeSurface === child.surface
                return (
                  <li key={child.surface}>
                    <button
                      type="button"
                      className={`${styles.projectTreeChildButton} ${isActive ? styles.projectTreeChildButtonActive : ""}`}
                      aria-current={isActive ? "page" : undefined}
                      onClick={() => onSurfaceSelect?.(project, child.surface)}
                      data-testid={`project-tree-${child.surface}`}
                    >
                      <NavigationIcon name={child.icon} size={17} />
                      <span className={styles.projectTreeChildLabel}>{child.label}</span>
                    </button>
                  </li>
                )
              })}
            </ul>
          ) : null}
        </li>
      </ul>
    </nav>
  )
}

export default ProjectTree
