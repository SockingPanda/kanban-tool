import { isValidElement, type ReactElement, type ReactNode } from "react"
import { describe, expect, it, vi } from "vitest"

import { DropdownMenuRadioGroup, DropdownMenuRadioItem } from "@/components/ui/dropdown-menu"

import { MenuSelect, type MenuSelectOption } from "./menu-select"

describe("MenuSelect", () => {
  it("wires the selected radio value and emits menu changes", () => {
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

    const radioGroup = findElement(tree, DropdownMenuRadioGroup)
    expect(radioGroup?.props.value).toBe("ready")
    expect(radioGroup?.props.onValueChange).toBeTypeOf("function")

    radioGroup?.props.onValueChange?.("blocked")
    expect(onValueChange).toHaveBeenCalledWith("blocked")

    const radioItems = findElements(tree, DropdownMenuRadioItem)
    expect(radioItems.map((item) => item.props.value)).toEqual(["all", "ready", "blocked"])
    expect(radioItems.map((item) => item.props.children)).toEqual(["all active", "ready", "blocked"])
  })
})

type InspectableProps = {
  children?: ReactNode
  value?: string
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
