import { isValidElement, type ReactElement, type ReactNode } from "react"
import { describe, expect, it, vi } from "vitest"

import { DependencyGroup } from "./TaskDetail"
import type { Task } from "@/lib/api"

describe("DependencyGroup", () => {
  it("opens dependency chips without conflicting with parent removal", () => {
    const parent = taskFixture({ id: "t_parent", seq: 12, title: "Parent task", status: "done" })
    const onSelect = vi.fn()
    const onRemove = vi.fn()

    const tree = DependencyGroup({
      title: "Parents",
      tasks: [parent],
      onSelect,
      onRemove,
    })

    const openButton = findButtonByLabel(tree, "Open parent dependency #12 Parent task")
    expect(openButton?.props.type).toBe("button")
    openButton?.props.onClick?.()
    expect(onSelect).toHaveBeenCalledWith("t_parent")

    const removeButton = findButtonByTitle(tree, "Remove parent dependency")
    expect(removeButton?.props.type).toBe("button")
    removeButton?.props.onClick?.()
    expect(onRemove).toHaveBeenCalledWith("t_parent")
    expect(onSelect).toHaveBeenCalledTimes(1)
  })

  it("opens child dependency chips with a clear accessible label", () => {
    const child = taskFixture({ id: "t_child", seq: 13, title: "Child task", status: "ready" })
    const onSelect = vi.fn()

    const tree = DependencyGroup({
      title: "Children",
      tasks: [child],
      onSelect,
    })

    const openButton = findButtonByLabel(tree, "Open child dependency #13 Child task")
    expect(openButton?.props.type).toBe("button")
    openButton?.props.onClick?.()
    expect(onSelect).toHaveBeenCalledWith("t_child")
  })
})

type ButtonProps = {
  "aria-label"?: string
  title?: string
  type?: "button" | "submit" | "reset"
  onClick?: () => void
  children?: ReactNode
}

function findButtonByLabel(node: ReactNode, label: string) {
  return findButtons(node).find((button) => button.props["aria-label"] === label) ?? null
}

function findButtonByTitle(node: ReactNode, title: string) {
  return findButtons(node).find((button) => button.props.title === title) ?? null
}

function findButtons(node: ReactNode): ReactElement<ButtonProps>[] {
  if (Array.isArray(node)) return node.flatMap((child) => findButtons(child))
  if (!isValidElement(node)) return []

  const element = node as ReactElement<ButtonProps>
  const matches = element.type === "button" ? [element] : []
  return matches.concat(findButtons(element.props.children))
}

function taskFixture(overrides: Pick<Task, "id" | "seq" | "title" | "status">): Task {
  return {
    id: overrides.id,
    board_id: "b_default",
    board_slug: "default",
    ref: `default#${overrides.seq}`,
    seq: overrides.seq,
    title: overrides.title,
    description: null,
    status: overrides.status,
    status_reason: null,
    assignee: null,
    priority: 3,
    position: 0,
    scheduled_at: null,
    due_at: null,
    created_by: "test",
    created_at: 1,
    updated_at: 1,
    started_at: null,
    completed_at: null,
    archived_at: null,
    claim_owner: null,
    claim_expires_at: null,
    last_heartbeat_at: null,
    current_run_id: null,
    retry_count: 0,
    max_retries: null,
    result_summary: null,
    result_json: null,
    metadata_json: "{}",
    lock_version: 0,
    dependency_blocked: false,
    unfinished_parent_count: 0,
  }
}
