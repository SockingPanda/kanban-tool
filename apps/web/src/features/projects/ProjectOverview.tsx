import type { MouseEvent } from "react"

import type { BoardListItem } from "../../lib/api/board-list-read-model"
import { routePath } from "../../lib/router"
import { createTranslator } from "../../lib/i18n"
import { usePreferences } from "../../lib/use-preferences"
import { NavigationIcon } from "../../ui/navigation"
import styles from "./projects.module.css"

export type ProjectOverviewStatus = "ready" | "offline" | "error" | "stale" | "recovering"

export type ProjectOverviewProps = {
  readonly project: BoardListItem
  readonly onOpenTasks?: () => void
  readonly basePath?: string
  readonly status?: ProjectOverviewStatus
  readonly onRetry?: () => void
}

/**
 * Identity-only project overview. No per-project request, metrics, activity,
 * ownership, cover or lifecycle controls belong on this surface.
 */
export function ProjectOverview({ project, onOpenTasks, basePath = "/app/", status = "ready", onRetry }: ProjectOverviewProps) {
  const { locale } = usePreferences()
  const t = createTranslator(locale)
  const statusCopy = status === "offline"
    ? t("projectCollectionOfflineDetail")
    : status === "error"
      ? t("projectCollectionErrorDetail")
      : status === "stale"
        ? t("projectCollectionStaleDetail")
        : status === "recovering"
          ? t("projectCollectionRecoveringDetail")
          : undefined
  return (
    <section className={styles.projectsSurface} aria-labelledby="project-overview-title" data-testid="project-overview" data-archived={project.archivedAt === null ? "false" : "true"}>
      <div className={styles.overviewLayout}>
        <div className={styles.overviewMain}>
          <h1 id="project-overview-title">{project.name}</h1>
          <p className={styles.overviewDescription}>{project.description ?? t("projectDescriptionFallback")}</p>
          {statusCopy !== undefined ? (
            <div className={styles.overviewBoundary} role={status === "error" || status === "offline" ? "alert" : "status"} data-testid="project-overview-status" data-status={status}>
              <span>{statusCopy}</span>
              {onRetry !== undefined ? <button type="button" onClick={onRetry}>{t("retry")}</button> : null}
            </div>
          ) : null}
          <dl className={styles.identityRows}>
            <div className={styles.identityRow}>
              <dt>{t("slug")}</dt>
              <dd><code translate="no">{project.slug}</code></dd>
            </div>
            <div className={styles.identityRow}>
              <dt>{t("archiveState")}</dt>
              <dd>{project.archivedAt === null ? t("active") : t("archived")}</dd>
            </div>
          </dl>
          {project.archivedAt === null ? (
            <a
              href={routePath({ kind: "board", boardSlug: project.slug, view: "board" }, { basePath })}
              className={styles.overviewTasksButton}
              onClick={(event: MouseEvent<HTMLAnchorElement>) => {
                if (onOpenTasks === undefined || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
                event.preventDefault()
                onOpenTasks()
              }}
              data-testid="project-overview-open-tasks"
            >
              <span><NavigationIcon name="list" size={17} />{t("tasks")}</span>
              <NavigationIcon name="chevron-right" size={16} />
            </a>
          ) : null}
        </div>
      </div>
    </section>
  )
}

export default ProjectOverview
