import type { ReactNode } from "react"

/**
 * Storybook and shell consumers pass the same identity-only board shape. The
 * navigation layer deliberately does not add lifecycle, metrics, or owner
 * fields that are not part of the canonical board contract.
 */
export type NavigationProject = {
  readonly id: string
  readonly slug: string
  readonly name: string
  readonly description?: string | null
  readonly archived?: boolean
}

export type ProjectSurface = "projects" | "overview" | "tasks"

export type ProjectPickerStatus = "ready" | "loading" | "offline" | "error"

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
  readonly retry: string
  readonly close: string
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
  home: "Home",
  projectSearch: "Search projects",
  projectSearchPlaceholder: "Search projects",
  clearSearch: "Clear project search",
  projectSearchEmpty: "No projects match this search.",
  projectsEmpty: "No canonical projects are available.",
  projectLoading: "Loading projects…",
  projectOffline: "Projects are unavailable while offline.",
  projectError: "Projects could not be loaded.",
  retry: "Retry",
  close: "Close",
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
