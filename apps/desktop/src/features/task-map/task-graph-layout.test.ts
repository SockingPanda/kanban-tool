import { describe, expect, it } from "vitest"

import { layoutTaskGraph } from "./task-graph-layout"
import type { TaskGraph } from "./task-graph-types"

const graph: TaskGraph = {
  nodes: [
    { id: "center", ref: "default#123", title: "Graph base", status: "running", role: "center" },
    { id: "parent", ref: "default#123", title: "Contract", status: "done", role: "dependency_parent", contextOnly: true },
    { id: "child", ref: "default#123", title: "Detail workbench", status: "todo", role: "dependency_child" },
    { id: "subtask", ref: "default#123", title: "Renderer", status: "todo", role: "subtask_child" },
  ],
  edges: [
    { id: "dep:parent:center", sourceTaskId: "parent", targetTaskId: "center", kind: "dependency", blocking: true },
    { id: "dep:center:child", sourceTaskId: "center", targetTaskId: "child", kind: "dependency", blocking: false },
    { id: "subtask:center:subtask", sourceTaskId: "center", targetTaskId: "subtask", kind: "subtask", required: true },
  ],
}

describe("layoutTaskGraph", () => {
  it("places the detail center between dependency parents, children, and subtasks", () => {
    const layout = layoutTaskGraph(graph, { mode: "detail", selectedTaskId: "center" })
    const center = layout.nodes.find((node) => node.id === "center")
    const parent = layout.nodes.find((node) => node.id === "parent")
    const child = layout.nodes.find((node) => node.id === "child")
    const subtask = layout.nodes.find((node) => node.id === "subtask")

    expect(parent?.x).toBeLessThan(center?.x ?? 0)
    expect(child?.x).toBeGreaterThan(center?.x ?? 0)
    expect(subtask?.y).toBeGreaterThan(center?.y ?? 0)
    expect(layout.edges.map((edge) => edge.id)).toEqual(["dep:parent:center", "dep:center:child", "subtask:center:subtask"])
  })

  it("keeps board-map layout deterministic by status then ref", () => {
    const first = layoutTaskGraph(graph, { mode: "board-map" })
    const second = layoutTaskGraph({ nodes: [...graph.nodes].reverse(), edges: graph.edges }, { mode: "board-map" })

    expect(first.nodes.map((node) => [node.id, node.x, node.y])).toEqual(second.nodes.map((node) => [node.id, node.x, node.y]))
  })
})
