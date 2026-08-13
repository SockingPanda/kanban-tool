import type { MouseEvent } from "react"
import { Badge } from "@astryxdesign/core/Badge"
import { Banner } from "@astryxdesign/core/Banner"
import { Code } from "@astryxdesign/core/Code"
import { Button } from "@astryxdesign/core/Button"
import { Heading } from "@astryxdesign/core/Heading"
import { Text } from "@astryxdesign/core/Text"

import type { BoardListItem } from "../../lib/api/board-list-read-model"
import { routePath } from "../../lib/router"
import { createTranslator } from "../../lib/i18n"
import { usePreferences } from "../../lib/use-preferences"
import { PageFrame } from "../../ui/astryx/page-frame/PageFrame"
import { StaticHStack, StaticMetadataList, StaticMetadataListItem, StaticVStack } from "../../ui/astryx/primitives/safe-core"
import { NavigationIcon } from "../../ui/navigation"

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
    ? { label: t("projectCollectionOffline"), detail: t("projectCollectionOfflineDetail") }
    : status === "error"
      ? { label: t("projectCollectionError"), detail: t("projectCollectionErrorDetail") }
      : status === "stale"
        ? { label: t("projectCollectionStale"), detail: t("projectCollectionStaleDetail") }
        : status === "recovering"
          ? { label: t("projectCollectionRecovering"), detail: t("projectCollectionRecoveringDetail") }
          : undefined
  const boundaryStatus = status === "offline" || status === "error" ? "error" : "warning"
  const boundaryRole = status === "offline" || status === "error" ? "alert" : "status"
  return (
    <StaticVStack
      as="section"
      data-testid="project-overview"
      data-archived={project.archivedAt === null ? "false" : "true"}
      className="min-w-0"
    >
      <PageFrame
        frame="content"
        aria-labelledby="project-overview-title"
        bodyLabel={t("overview")}
        header={(
          <StaticVStack gap={1} className="mx-auto w-full max-w-6xl border-b border-border pb-3">
            <Heading level={1} id="project-overview-title">{project.name}</Heading>
            <Text as="p" type="body" color="secondary">{project.description ?? t("projectDescriptionFallback")}</Text>
          </StaticVStack>
        )}
      >
        <StaticVStack gap={5} className="mx-auto w-full max-w-6xl">
          {statusCopy !== undefined ? (
            <Banner
              status={boundaryStatus}
              role={boundaryRole}
              aria-live="polite"
              title={statusCopy.label}
              description={statusCopy.detail}
              container="section"
              endContent={onRetry !== undefined && status !== "recovering" ? <Button label={t("retry")} variant="secondary" size="sm" onClick={onRetry} /> : undefined}
              data-testid="project-overview-status"
              data-status={status}
            />
          ) : null}
          <StaticMetadataList data-testid="project-overview-identity">
            <StaticMetadataListItem label={t("slug")}><span translate="no"><Code>{project.slug}</Code></span></StaticMetadataListItem>
            <StaticMetadataListItem label={t("archiveState")}>
              <Badge
                variant={project.archivedAt === null ? "info" : "neutral"}
                label={project.archivedAt === null ? t("active") : t("archived")}
                data-testid="project-overview-archive-state"
              />
            </StaticMetadataListItem>
          </StaticMetadataList>
          {project.archivedAt === null ? (
            <a
              href={routePath({ kind: "board", boardSlug: project.slug, view: "board" }, { basePath })}
              data-testid="project-overview-open-tasks"
              className="flex w-full items-center justify-between gap-3 rounded-md border border-border-strong bg-surface px-3 py-3 text-start text-primary no-underline hover:border-accent hover:bg-muted focus-visible:outline-2 focus-visible:outline-accent"
              onClick={onOpenTasks === undefined ? undefined : (event: MouseEvent<HTMLAnchorElement>) => {
                if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
                event.preventDefault()
                onOpenTasks()
              }}
            >
              <StaticHStack gap={2} align="center" className="min-w-0">
                <NavigationIcon name="list" size={17} />
                <Text>{t("tasks")}</Text>
              </StaticHStack>
              <NavigationIcon name="chevron-right" size={16} />
            </a>
          ) : null}
        </StaticVStack>
      </PageFrame>
    </StaticVStack>
  )
}

export default ProjectOverview
