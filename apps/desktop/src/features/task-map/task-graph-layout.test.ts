import { describe, expect, it } from "vitest"

import { layoutTaskGraph } from "./task-graph-layout"
import type { TaskGraph } from "./task-graph-types"

const graph: TaskGraph = {
  nodes: [
    { id: "center", ref: "default#123", title: "Graph base", status: "running", role: "center" },
    { id: "parent", ref: "default#123", title: "Contract", status: "done", role: "dependency_parent", contextOnly: true },
    { id: "child", ref: "default#123", title: "Detail workbench", status: "todo", role: "dependency_child" },
    { id: "step", ref: "default#123", title: "Renderer", status: "todo", role: "step_child" },
  ],
  edges: [
    { id: "dep:parent:center", sourceTaskId: "parent", targetTaskId: "center", kind: "dependency", blocking: true },
    { id: "dep:center:child", sourceTaskId: "center", targetTaskId: "child", kind: "dependency", blocking: false },
    { id: "step:center:step", sourceTaskId: "center", targetTaskId: "step", kind: "step", required: true },
  ],
}

describe("layoutTaskGraph", () => {
  it("places the detail center between dependency parents, children, and steps", () => {
    const layout = layoutTaskGraph(graph, { mode: "detail", selectedTaskId: "center" })
    const center = layout.nodes.find((node) => node.id === "center")
    const parent = layout.nodes.find((node) => node.id === "parent")
    const child = layout.nodes.find((node) => node.id === "child")
    const step = layout.nodes.find((node) => node.id === "step")

    expect(parent?.x).toBeLessThan(center?.x ?? 0)
    expect(child?.x).toBeGreaterThan(center?.x ?? 0)
    expect(step?.y).toBeGreaterThan(center?.y ?? 0)
    expect(layout.edges.map((edge) => edge.id)).toEqual(["dep:parent:center", "dep:center:child", "step:center:step"])
  })

  it("keeps board-map layout deterministic by status then ref", () => {
    const first = layoutTaskGraph(graph, { mode: "board-map" })
    const second = layoutTaskGraph({ nodes: [...graph.nodes].reverse(), edges: graph.edges }, { mode: "board-map" })

    expect(first.nodes.map((node) => [node.id, node.x, node.y])).toEqual(second.nodes.map((node) => [node.id, node.x, node.y]))
  })

  it("returns finite dimensions for an empty graph", () => {
    const layout = layoutTaskGraph({ nodes: [], edges: [] }, { mode: "board-map" })

    expect(layout.nodes).toEqual([])
    expect(layout.edges).toEqual([])
    expect(Number.isFinite(layout.width)).toBe(true)
    expect(Number.isFinite(layout.height)).toBe(true)
    expect(layout.width).toBeGreaterThan(0)
    expect(layout.height).toBeGreaterThan(0)
  })

  it("routes reverse board-map edges from the source left side to the target right side", () => {
    const reverseLayout = layoutTaskGraph(
      {
        nodes: [
          { id: "ready", ref: "default#123", title: "Ready", status: "ready", role: "context" },
          { id: "done", ref: "default#123", title: "Done context", status: "done", role: "context", contextOnly: true },
        ],
        edges: [{ id: "dep:done:ready", sourceTaskId: "done", targetTaskId: "ready", kind: "dependency", blocking: false }],
      },
      { mode: "board-map" },
    )
    const source = reverseLayout.nodes.find((node) => node.id === "done")
    const target = reverseLayout.nodes.find((node) => node.id === "ready")
    const edge = reverseLayout.edges[0]

    expect(source).toBeTruthy()
    expect(target).toBeTruthy()
    expect(edge.path.startsWith(`M ${source?.x} ${source ? source.y + source.height / 2 : 0}`)).toBe(true)
    expect(edge.path.endsWith(`${target ? target.x + target.width : 0} ${target ? target.y + target.height / 2 : 0}`)).toBe(true)
  })

  it("routes same-column edges vertically instead of looping through side anchors", () => {
    const sameColumnLayout = layoutTaskGraph(
      {
        nodes: [
          { id: "first", ref: "default#123", title: "First", status: "ready", role: "context" },
          { id: "second", ref: "default#123", title: "Second", status: "ready", role: "context" },
        ],
        edges: [{ id: "dep:first:second", sourceTaskId: "first", targetTaskId: "second", kind: "dependency", blocking: false }],
      },
      { mode: "board-map" },
    )
    const source = sameColumnLayout.nodes.find((node) => node.id === "first")
    const target = sameColumnLayout.nodes.find((node) => node.id === "second")
    const edge = sameColumnLayout.edges[0]

    expect(source).toBeTruthy()
    expect(target).toBeTruthy()
    expect(edge.path.startsWith(`M ${source ? source.x + source.width / 2 : 0} ${source ? source.y + source.height : 0}`)).toBe(true)
    expect(edge.path.endsWith(`${target ? target.x + target.width / 2 : 0} ${target?.y}`)).toBe(true)
  })
})
