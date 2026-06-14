import { isValidElement, type ReactElement, type ReactNode } from "react"
import { describe, expect, it, vi } from "vitest"

import { Select, SelectItem, SelectTrigger } from "@/components/ui/select"

import { MenuSelect, type MenuSelectOption } from "./menu-select"

describe("MenuSelect", () => {
  it("wires the selected value and emits select changes with accessible trigger label", () => {
    const options: MenuSelectOption<"all" | "ready" | "blocked">[] = [
      { value: "all", label: "all active" },
      { value: "ready", label: "ready" },
      { value: "blocked", label: "blocked" },
    ]
    const onValueChange = vi.fn()

    const tree = MenuSelect({
      ariaLabel: "Status filter",
      prefix: "Status",
      value: "ready",
      options,
      onValueChange,
    })

    const select = findElement(tree, Select)
    expect(select?.props.value).toBe("ready")
    expect(select?.props.onValueChange).toBeTypeOf("function")

    select?.props.onValueChange?.("blocked")
    expect(onValueChange).toHaveBeenCalledWith("blocked")

    const trigger = findElement(tree, SelectTrigger)
    expect(trigger?.props["aria-label"]).toBe("Status filter")

    const items = findElements(tree, SelectItem)
    expect(items.map((item) => item.props.value)).toEqual(["all", "ready", "blocked"])
    expect(items.map((item) => item.props.children)).toEqual(["all active", "ready", "blocked"])
  })
})

type InspectableProps = {
  children?: ReactNode
  value?: string
  "aria-label"?: string
  onValueChange?: (value: string) => void
}

function findElement(node: ReactNode, type: unknown): ReactElement<InspectableProps> | null {
  return findElements(node, type)[0] ?? null
}

function findElements(node: ReactNode, type: unknown): ReactElement<InspectableProps>[] {
  if (Array.isArray(node)) return node.flatMap((child) => findElements(child, type))
  if (!isValidElement(node)) return []

  const element = node as ReactElement<InspectableProps>
  const matches = element.type === type ? [element] : []
  return matches.concat(findElements(element.props.children, type))
}
