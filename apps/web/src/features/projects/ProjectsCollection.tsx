import { useMemo, useState, type MouseEvent } from "react"
import { Banner } from "@astryxdesign/core/Banner"
import { Button } from "@astryxdesign/core/Button"
import { Heading } from "@astryxdesign/core/Heading"
import { List, ListItem } from "@astryxdesign/core/List"
import { StackItem } from "@astryxdesign/core/Stack"
import { Text } from "@astryxdesign/core/Text"

import type { BoardListItem } from "../../lib/api/board-list-read-model"
import { routePath } from "../../lib/router"
import { createTranslator } from "../../lib/i18n"
import { usePreferences } from "../../lib/use-preferences"
import { TextInput } from "../../ui/astryx/fields/TextInput"
import { PageFrame } from "../../ui/astryx/page-frame/PageFrame"
import { StaticHStack, StaticVStack } from "../../ui/astryx/primitives/safe-core"
import { NavigationIcon } from "../../ui/navigation"

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

function statusCopy(status: ProjectsCollectionStatus, hasSnapshot: boolean, t: ReturnType<typeof createTranslator>): { readonly label: string; readonly detail: string } {
  if (status === "loading") return { label: t("projectCollectionLoading"), detail: t("projectCollectionLoadingDetail") }
  if (status === "offline") return { label: t("projectCollectionOffline"), detail: hasSnapshot ? t("projectCollectionOfflineDetail") : t("projectCollectionOfflineNoSnapshotDetail") }
  if (status === "error") return { label: t("projectCollectionError"), detail: hasSnapshot ? t("projectCollectionErrorDetail") : t("projectCollectionErrorNoSnapshotDetail") }
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
  const hasSnapshot = projects.length > 0 || status === "ready"
  const copy = statusCopy(status, hasSnapshot, t)
  const showBoundary = status !== "ready" || activeProjects.length === 0
  const empty = status === "ready" && activeProjects.length === 0

  const boundaryStatus = status === "error" || status === "offline" ? "error" : status === "stale" || status === "recovering" ? "warning" : "info"
  const boundaryRole = status === "error" || status === "offline" ? "alert" : "status"
  const boundaryTitle = empty
    ? query.trim().length > 0
      ? `${t("projectSearchEmpty")}。`
      : t("projectsEmpty")
    : copy.detail

  return (
    <StaticVStack
      as="section"
      data-testid="projects-collection"
      data-status={status}
      data-has-snapshot={hasSnapshot ? "true" : "false"}
      className="min-w-0"
    >
      <PageFrame
        frame="content"
        aria-labelledby="projects-collection-title"
        bodyLabel={t("projects")}
        header={(
          <StaticVStack gap={1} className="mx-auto w-full max-w-6xl border-b border-border pb-3">
            <StaticHStack gap={4} justify="between" align="end" wrap="wrap">
              <StackItem size="fill">
                <StaticVStack gap={1}>
                  <Heading level={1} id="projects-collection-title">{copy.label}</Heading>
                  <Text as="p" type="body" color="secondary">{copy.detail}</Text>
                </StaticVStack>
              </StackItem>
              <StackItem size="fill" className="min-w-0">
                <TextInput
                  id="projects-search"
                  data-testid="projects-search"
                  label={t("projectSearch")}
                  isLabelHidden
                  value={query}
                  onChange={(value) => setQuery(value)}
                  type="search"
                  placeholder={t("projectSearchPlaceholder")}
                  hasClear
                  clearLabel={t("clearSearch")}
                  clearText={t("clearSearch")}
                  isDisabled={status === "loading" && !hasSnapshot}
                />
              </StackItem>
            </StaticHStack>
          </StaticVStack>
        )}
      >
        <StaticVStack gap={5} className="mx-auto w-full max-w-6xl">
          {isRefreshing ? <Text as="p" type="supporting" role="status">{t("projectsRefreshing")}</Text> : null}
          {showBoundary ? (
            <Banner
              status={boundaryStatus}
              role={boundaryRole}
              aria-live="polite"
              title={boundaryTitle}
              container="section"
              endContent={onRetry !== undefined && status !== "loading" && status !== "recovering" && !isRefreshing ? <Button label={t("retry")} variant="secondary" size="sm" onClick={onRetry} /> : undefined}
              data-testid={"projects-collection-" + (empty ? "empty" : status)}
            />
          ) : null}

          {activeProjects.length > 0 ? (
            <List density="spacious" hasDividers data-testid="projects-collection-list">
              {activeProjects.map((project) => (
                <ListItem
                  key={project.id}
                  label={(
                    <a
                      href={routePath({ kind: "project-overview", boardSlug: project.slug }, { basePath })}
                      data-testid={"projects-collection-project-" + project.slug}
                      className="flex min-w-0 items-center gap-3 rounded-md px-3 py-3 text-start text-primary no-underline hover:bg-muted focus-visible:outline-2 focus-visible:outline-accent"
                      onClick={onOpenProject === undefined ? undefined : (event: MouseEvent<HTMLAnchorElement>) => {
                        if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
                        event.preventDefault()
                        onOpenProject(project)
                      }}
                    >
                      <NavigationIcon name="folder" size={22} />
                      <StaticVStack gap={1} className="min-w-0 flex-1">
                        <Text as="span" type="body" weight="semibold" className="truncate">{project.name}</Text>
                        <Text as="span" type="code" color="secondary" className="truncate">{project.slug}</Text>
                        {project.description !== null ? <Text as="span" type="supporting" color="secondary" className="truncate">{project.description}</Text> : null}
                      </StaticVStack>
                      <NavigationIcon name="chevron-right" size={17} />
                    </a>
                  )}
                />
              ))}
            </List>
          ) : null}
        </StaticVStack>
      </PageFrame>
    </StaticVStack>
  )
}

export default ProjectsCollection
