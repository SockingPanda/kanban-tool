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
})

type ButtonProps = {
  "aria-label"?: string
  "aria-pressed"?: boolean
  type?: "button" | "submit" | "reset"
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
