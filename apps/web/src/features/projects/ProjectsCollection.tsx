import { useMemo, useState, type ChangeEvent, type MouseEvent } from "react"

import type { BoardListItem } from "../../lib/api/board-list-read-model"
import { routePath } from "../../lib/router"
import { createTranslator } from "../../lib/i18n"
import { usePreferences } from "../../lib/use-preferences"
import { NavigationIcon } from "../../ui/navigation"
import styles from "./projects.module.css"

export type ProjectsCollectionStatus = "loading" | "ready" | "offline" | "error" | "stale" | "recovering"

export type ProjectsCollectionProps = {
  readonly projects: readonly BoardListItem[]
  readonly status?: ProjectsCollectionStatus
  readonly isRefreshing?: boolean
  readonly onRetry?: () => void
  readonly onOpenProject?: (project: BoardListItem) => void
  /** Used by the progressive project links when the SPA callback is absent. */
  readonly basePath?: string
}

function statusCopy(status: ProjectsCollectionStatus, t: ReturnType<typeof createTranslator>): { readonly label: string; readonly detail: string } {
  if (status === "loading") return { label: t("projectCollectionLoading"), detail: t("projectCollectionLoadingDetail") }
  if (status === "offline") return { label: t("projectCollectionOffline"), detail: t("projectCollectionOfflineDetail") }
  if (status === "error") return { label: t("projectCollectionError"), detail: t("projectCollectionErrorDetail") }
  if (status === "stale") return { label: t("projectCollectionStale"), detail: t("projectCollectionStaleDetail") }
  if (status === "recovering") return { label: t("projectCollectionRecovering"), detail: t("projectCollectionRecoveringDetail") }
  return { label: t("projects"), detail: t("projectCollectionReadyDetail") }
}

function matches(project: BoardListItem, query: string): boolean {
  const normalized = query.trim().toLocaleLowerCase()
  if (normalized.length === 0) return true
  return [project.name, project.slug, project.description ?? ""].some((value) => value.toLocaleLowerCase().includes(normalized))
}

/**
 * Production Projects collection. It intentionally consumes only the global
 * BoardListItem snapshot; archived projects remain reachable by explicit deep
 * links but do not appear in the default collection.
 */
export function ProjectsCollection({
  projects,
  status = "ready",
  isRefreshing = false,
  onRetry,
  onOpenProject,
  basePath = "/app/",
}: ProjectsCollectionProps) {
  const { locale } = usePreferences()
  const t = createTranslator(locale)
  const [query, setQuery] = useState("")
  const activeProjects = useMemo(
    () => projects.filter((project) => project.archivedAt === null && matches(project, query)),
    [projects, query],
  )
  const copy = statusCopy(status, t)
  const hasSnapshot = projects.length > 0 || status === "ready"
  const showBoundary = status !== "ready" || activeProjects.length === 0
  const empty = status === "ready" && activeProjects.length === 0

  const onSearch = (event: ChangeEvent<HTMLInputElement>) => setQuery(event.currentTarget.value)

  return (
    <section className={styles.projectsSurface} aria-labelledby="projects-collection-title" data-testid="projects-collection" data-status={status} data-has-snapshot={hasSnapshot ? "true" : "false"}>
      <div className={styles.projectsHeading}>
        <div>
          <h1 id="projects-collection-title">{copy.label}</h1>
          <p>{copy.detail}</p>
        </div>
        <label className={styles.projectsSearch}>
          <NavigationIcon name="search" size={16} />
          <span className={styles.visuallyHidden}>{t("projectSearch")}</span>
          <input type="search" value={query} onChange={onSearch} placeholder={t("projectSearch")} aria-label={t("projectSearch")} disabled={status === "loading" && !hasSnapshot} />
        </label>
      </div>

      {isRefreshing ? <p className={styles.projectsRefreshing} role="status">{t("projectsRefreshing")}</p> : null}
      {showBoundary ? (
        <div className={styles.projectsBoundary + (empty ? " " + styles.projectsBoundaryEmpty : "")} role={status === "error" || status === "offline" ? "alert" : "status"} aria-live="polite" data-testid={"projects-collection-" + (empty ? "empty" : status)}>
          <span>{empty ? (query.trim().length > 0 ? `${t("projectSearchEmpty")}。` : t("projectsEmpty")) : copy.detail}</span>
          {onRetry !== undefined && status !== "loading" && !isRefreshing ? <button type="button" onClick={onRetry}>{t("retry")}</button> : null}
        </div>
      ) : null}

      {activeProjects.length > 0 ? (
        <ul className={styles.projectRows} data-testid="projects-collection-list">
          {activeProjects.map((project) => (
            <li key={project.id}>
              <a
                href={routePath({ kind: "project-overview", boardSlug: project.slug }, { basePath })}
                className={styles.projectRow}
                onClick={(event: MouseEvent<HTMLAnchorElement>) => {
                  if (onOpenProject === undefined || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
                  event.preventDefault()
                  onOpenProject(project)
                }}
                data-testid={"projects-collection-project-" + project.slug}
              >
                <NavigationIcon name="folder" size={22} />
                <span className={styles.projectRowCopy}>
                  <span className={styles.projectRowName}>{project.name}</span>
                  <span className={styles.projectRowSlug}>{project.slug}</span>
                </span>
                {project.description !== null ? <span className={styles.projectRowDescription}>{project.description}</span> : null}
                <NavigationIcon name="chevron-right" size={17} />
              </a>
            </li>
          ))}
        </ul>
      ) : null}
    </section>
  )
}

export default ProjectsCollection
