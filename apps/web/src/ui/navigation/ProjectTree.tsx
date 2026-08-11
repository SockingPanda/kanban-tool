import { useState, type KeyboardEvent, type MouseEvent } from "react"

import { routePath } from "../../lib/router"
import { NavigationIcon } from "./icons"
import styles from "./navigation.module.css"
import { mergeNavigationLabels, type NavigationLabels, type NavigationProject, type ProjectSurface } from "./types"

export type ProjectTreeProps = {
  readonly project: NavigationProject
  readonly activeSurface?: ProjectSurface
  readonly expanded?: boolean
  readonly defaultExpanded?: boolean
  readonly basePath?: string
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
  basePath = "/app/",
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
    ...(project.archivedAt === null ? [{ surface: "tasks" as const, label: labels.tasks, icon: "list" as const }] : []),
  ]

  return (
    <nav className={rootClassName} aria-label={`${project.name} navigation`} data-testid="project-tree">
      <h3 className={styles.projectTreeHeading}>
        <span>{project.name}</span>
        {project.archivedAt !== null ? <span aria-label={labels.archived}>· {labels.archived}</span> : null}
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
            <a
              className={styles.projectTreeProjectButton}
              href={routePath({ kind: "project-overview", boardSlug: project.slug }, { basePath })}
              onClick={(event: MouseEvent<HTMLAnchorElement>) => {
                if (onProjectSelect === undefined || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
                event.preventDefault()
                onProjectSelect(project)
              }}
              data-testid="project-tree-project"
            >
              <NavigationIcon name="folder" size={17} />
              <span className={styles.projectTreeProjectName}>{project.name}</span>
            </a>
          </div>
          {isExpanded ? (
            <ul id={childrenId} className={styles.projectTreeChildren}>
              {children.map((child) => {
                const isActive = activeSurface === child.surface
                const href = child.surface === "overview"
                  ? routePath({ kind: "project-overview", boardSlug: project.slug }, { basePath })
                  : routePath({ kind: "board", boardSlug: project.slug, view: "board" }, { basePath })
                return (
                  <li key={child.surface}>
                    <a
                      className={`${styles.projectTreeChildButton} ${isActive ? styles.projectTreeChildButtonActive : ""}`}
                      aria-current={isActive ? "page" : undefined}
                      href={href}
                      onClick={(event: MouseEvent<HTMLAnchorElement>) => {
                        if (onSurfaceSelect === undefined || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
                        event.preventDefault()
                        onSurfaceSelect(project, child.surface)
                      }}
                      data-testid={`project-tree-${child.surface}`}
                    >
                      <NavigationIcon name={child.icon} size={17} />
                      <span className={styles.projectTreeChildLabel}>{child.label}</span>
                    </a>
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
