import type { MouseEvent } from "react"
import { Banner } from "@astryxdesign/core/Banner"
import { ClickableCard } from "@astryxdesign/core/ClickableCard"
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
    ? t("projectCollectionOfflineDetail")
    : status === "error"
      ? t("projectCollectionErrorDetail")
      : status === "stale"
        ? t("projectCollectionStaleDetail")
        : status === "recovering"
          ? t("projectCollectionRecoveringDetail")
          : undefined
  const boundaryStatus = status === "offline" || status === "error" ? "error" : "warning"
  const boundaryRole = status === "offline" || status === "error" ? "alert" : "status"
  return (
    <StaticVStack
      as="section"
      aria-labelledby="project-overview-title"
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
              title={statusCopy}
              container="section"
              endContent={onRetry !== undefined ? <Button label={t("retry")} variant="secondary" size="sm" onClick={onRetry} /> : undefined}
              data-testid="project-overview-status"
              data-status={status}
            />
          ) : null}
          <StaticMetadataList data-testid="project-overview-identity">
            <StaticMetadataListItem label={t("slug")}><Code>{project.slug}</Code></StaticMetadataListItem>
            <StaticMetadataListItem label={t("archiveState")}>
              {project.archivedAt === null ? t("active") : t("archived")}
            </StaticMetadataListItem>
          </StaticMetadataList>
          {project.archivedAt === null ? (
            <ClickableCard
              label={t("tasks")}
              href={routePath({ kind: "board", boardSlug: project.slug, view: "board" }, { basePath })}
              className="w-full"
              onClick={onOpenTasks === undefined ? undefined : (event: MouseEvent<HTMLElement>) => {
                if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
                event.preventDefault()
                onOpenTasks()
              }}
              onClickCapture={onOpenTasks === undefined ? undefined : (event: MouseEvent<HTMLElement>) => {
                const target = event.target
                if (!(target instanceof Element) || target.closest("a") === null) return
                if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
                event.preventDefault()
                event.stopPropagation()
                onOpenTasks()
              }}
              padding={3}
              data-testid="project-overview-open-tasks"
            >
              <StaticHStack justify="between" align="center" className="w-full">
                <StaticHStack gap={2} align="center">
                  <NavigationIcon name="list" size={17} />
                  <Text>{t("tasks")}</Text>
                </StaticHStack>
                <NavigationIcon name="chevron-right" size={16} />
              </StaticHStack>
            </ClickableCard>
          ) : null}
        </StaticVStack>
      </PageFrame>
    </StaticVStack>
  )
}

export default ProjectOverview
