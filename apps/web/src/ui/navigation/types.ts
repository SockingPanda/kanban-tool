import type { ReactNode } from "react"

import type { BoardListItem } from "../../lib/api/board-list-read-model"

/**
 * Storybook and shell consumers pass the same identity-only board shape. The
 * navigation layer deliberately does not add lifecycle, metrics, or owner
 * fields that are not part of the canonical board contract.
 */
/** Navigation consumes the canonical board list read model without weakening identity fields. */
export type NavigationProject = BoardListItem

export type ProjectSurface = "projects" | "overview" | "tasks"

export type ProjectPickerStatus = "ready" | "loading" | "offline" | "error" | "stale" | "recovering"

export type ContextNavigationSection = "home" | "projects" | "project"

export type BreadcrumbItem = {
  readonly label: string
  readonly href?: string
}

export type ResourceHeaderAction = {
  readonly id: string
  readonly label: string
  readonly kind?: "primary" | "secondary" | "ghost"
  readonly icon?: ReactNode
  readonly onSelect?: () => void
  readonly disabled?: boolean
}

export type ResourceHeaderMoreItem = {
  readonly id: string
  readonly label: string
  readonly href?: string
  readonly icon?: ReactNode
  readonly disabled?: boolean
  readonly onSelect?: () => void
}

export type NavigationLabels = {
  readonly productNavigation: string
  readonly projects: string
  readonly settings: string
  readonly home: string
  readonly projectSearch: string
  readonly projectSearchPlaceholder: string
  readonly clearSearch: string
  readonly projectSearchEmpty: string
  readonly projectsEmpty: string
  readonly projectLoading: string
  readonly projectOffline: string
  readonly projectError: string
  readonly projectStale: string
  readonly projectRecovering: string
  readonly retry: string
  readonly close: string
  readonly openProjectNavigation: string
  readonly closeProjectNavigation: string
  readonly collapse: string
  readonly expand: string
  readonly overview: string
  readonly tasks: string
  readonly more: string
  readonly breadcrumb: string
  readonly projectPicker: string
}

export const defaultNavigationLabels: NavigationLabels = {
  productNavigation: "Product navigation",
  projects: "Projects",
  settings: "Settings",
  home: "Home (planned)",
  projectSearch: "Search projects",
  projectSearchPlaceholder: "Search projects",
  clearSearch: "Clear project search",
  projectSearchEmpty: "No projects match this search.",
  projectsEmpty: "No canonical projects are available.",
  projectLoading: "Loading projects…",
  projectOffline: "Projects are unavailable while offline.",
  projectError: "Projects could not be loaded.",
  projectStale: "Showing a cached project list.",
  projectRecovering: "Refreshing projects…",
  retry: "Retry",
  close: "Close",
  openProjectNavigation: "Open project navigation",
  closeProjectNavigation: "Close project navigation",
  collapse: "Collapse project navigation",
  expand: "Expand project navigation",
  overview: "Overview",
  tasks: "Tasks",
  more: "More",
  breadcrumb: "Breadcrumb",
  projectPicker: "Project switcher",
}

export function mergeNavigationLabels(labels?: Partial<NavigationLabels>): NavigationLabels {
  return { ...defaultNavigationLabels, ...labels }
}
