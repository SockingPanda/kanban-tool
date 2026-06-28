import { isValidElement, type ReactElement, type ReactNode } from "react"
import { describe, expect, it, vi } from "vitest"

import { TaskGraphNodeCard } from "./TaskGraphNodeCard"
import type { TaskGraphLayoutNode } from "./task-graph-types"

describe("TaskGraphNodeCard", () => {
  it("marks selected nodes and opens the selected task", () => {
    const onSelectTask = vi.fn()
    const node = nodeFixture({ id: "t_center", ref: "kanban-tool#286", title: "Graph base" })

    const tree = TaskGraphNodeCard({ node, selected: true, onSelectTask })
    const button = findButton(tree)

    expect(button?.props["aria-pressed"]).toBe(true)
    expect(button?.props["aria-label"]).toBe("Open task kanban-tool#286 Graph base")
    button?.props.onClick?.()
    expect(onSelectTask).toHaveBeenCalledWith("t_center")
  })

  it("uses dependency blocked styling for non-blocked nodes", () => {
    const node = nodeFixture({ id: "t_blocked_by_dependency", ref: "kanban-tool#304", title: "Blocked by dependency" })
    const tree = TaskGraphNodeCard({
      node: { ...node, status: "ready", dependencyBlocked: true },
      selected: false,
    })
    const button = findButton(tree)

    expect(button?.props.className).toContain("red")
    expect(button?.props.className).not.toContain("emerald")
    expect(textContent(tree)).toContain("ready")
  })

  it("shows completed step progress instead of open step counts", () => {
    const node = nodeFixture({ id: "t_center", ref: "kanban-tool#286", title: "Graph base" })
    const tree = TaskGraphNodeCard({
      node: { ...node, stepCounts: { completed: 3, total: 4 } },
      selected: false,
    })

    const text = textContent(tree)

    expect(text).toContain("3/4 step")
    expect(text).not.toContain("open")
  })

  it("fills nodes from the left by completed step progress", () => {
    const node = nodeFixture({ id: "t_center", ref: "kanban-tool#286", title: "Graph base" })
    const tree = TaskGraphNodeCard({
      node: { ...node, stepCounts: { completed: 3, total: 4 } },
      selected: false,
    })

    const progress = findByTestId(tree, "task-graph-node-step-progress")

    expect(progress?.props["aria-hidden"]).toBe(true)
    expect(progress?.props.style).toMatchObject({ width: "75%" })
  })

  it("clamps step progress fill to the node width", () => {
    const node = nodeFixture({ id: "t_center", ref: "kanban-tool#286", title: "Graph base" })
    const overComplete = TaskGraphNodeCard({
      node: { ...node, stepCounts: { completed: 8, total: 4 } },
      selected: false,
    })
    const negativeComplete = TaskGraphNodeCard({
      node: { ...node, stepCounts: { completed: -2, total: 4 } },
      selected: false,
    })

    expect(findByTestId(overComplete, "task-graph-node-step-progress")?.props.style).toMatchObject({ width: "100%" })
    expect(findByTestId(negativeComplete, "task-graph-node-step-progress")).toBeNull()
  })

  it("does not render progress fill when step counts are unavailable", () => {
    const node = nodeFixture({ id: "t_center", ref: "kanban-tool#286", title: "Graph base" })
    const withoutCounts = TaskGraphNodeCard({ node, selected: false })
    const emptyCounts = TaskGraphNodeCard({
      node: { ...node, stepCounts: { completed: 0, total: 0 } },
      selected: false,
    })

    expect(findByTestId(withoutCounts, "task-graph-node-step-progress")).toBeNull()
    expect(findByTestId(emptyCounts, "task-graph-node-step-progress")).toBeNull()
  })
})

type ButtonProps = {
  "aria-label"?: string
  "aria-pressed"?: boolean
  type?: "button" | "submit" | "reset"
  className?: string
  onClick?: () => void
  children?: ReactNode
}

function findButton(node: ReactNode): ReactElement<ButtonProps> | null {
  if (Array.isArray(node)) {
    for (const child of node) {
      const match = findButton(child)
      if (match) return match
    }
    return null
  }
  if (!isValidElement(node)) return null
  const element = node as ReactElement<ButtonProps>
  if (element.props.type === "button") return element
  return findButton(element.props.children)
}

function textContent(node: ReactNode): string {
  if (Array.isArray(node)) return node.map(textContent).join("")
  if (node === null || node === undefined || typeof node === "boolean") return ""
  if (typeof node === "string" || typeof node === "number") return String(node)
  if (!isValidElement(node)) return ""
  return textContent((node as ReactElement<{ children?: ReactNode }>).props.children)
}

function findByTestId(node: ReactNode, testId: string): ReactElement<Record<string, unknown>> | null {
  if (Array.isArray(node)) {
    for (const child of node) {
      const match = findByTestId(child, testId)
      if (match) return match
    }
    return null
  }
  if (!isValidElement(node)) return null
  const element = node as ReactElement<Record<string, unknown> & { children?: ReactNode }>
  if (element.props["data-testid"] === testId) return element
  return findByTestId(element.props.children, testId)
}

function nodeFixture(overrides: Pick<TaskGraphLayoutNode, "id" | "ref" | "title">): TaskGraphLayoutNode {
  return {
    id: overrides.id,
    ref: overrides.ref,
    title: overrides.title,
    status: "running",
    role: "center",
    x: 24,
    y: 24,
    width: 176,
    height: 72,
  }
}
