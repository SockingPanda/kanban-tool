import { describe, expect, it } from "vitest"

import { layoutTaskGraphFallback, layoutTaskGraphWithElk, shouldUseFallbackLayout } from "./task-graph-layout"
import type { TaskGraph } from "./task-graph-types"

const graph: TaskGraph = {
  nodes: [
    { id: "center", ref: "default#123", title: "Graph base", status: "running", role: "center" },
    { id: "parent", ref: "default#122", title: "Contract", status: "done", role: "dependency_parent", contextOnly: true },
    { id: "child", ref: "default#124", title: "Detail workbench", status: "todo", role: "dependency_child" },
    { id: "step", ref: "default#999", title: "Renderer", status: "todo", role: "step_child" },
  ],
  edges: [
    { id: "dep:parent:center", sourceTaskId: "parent", targetTaskId: "center", kind: "dependency", blocking: true },
    { id: "dep:center:child", sourceTaskId: "center", targetTaskId: "child", kind: "dependency", blocking: false },
    { id: "step:center:step", sourceTaskId: "center", targetTaskId: "step", kind: "step", required: true },
  ],
}

describe("layoutTaskGraphWithElk", () => {
  it("layers detail nodes by visible edge direction", async () => {
    const layout = await layoutTaskGraphWithElk(graph, { mode: "detail", selectedTaskId: "center" })
    const center = layout.nodes.find((node) => node.id === "center")
    const parent = layout.nodes.find((node) => node.id === "parent")
    const child = layout.nodes.find((node) => node.id === "child")
    const step = layout.nodes.find((node) => node.id === "step")

    expect(parent?.x).toBeLessThan(center?.x ?? 0)
    expect(child?.x).toBeGreaterThan(center?.x ?? 0)
    expect(step?.x).toBeGreaterThan(center?.x ?? 0)
    expect(layout.edges.map((edge) => edge.id)).toEqual(["dep:parent:center", "dep:center:child", "step:center:step"])
  })

  it("keeps board-map layout deterministic independent of input order", async () => {
    const first = await layoutTaskGraphWithElk(graph, { mode: "board-map" })
    const second = await layoutTaskGraphWithElk({ nodes: [...graph.nodes].reverse(), edges: graph.edges }, { mode: "board-map" })

    expect(first.nodes.map((node) => [node.id, node.x, node.y])).toEqual(second.nodes.map((node) => [node.id, node.x, node.y]))
  })

  it("returns finite dimensions for an empty graph", async () => {
    const layout = await layoutTaskGraphWithElk({ nodes: [], edges: [] }, { mode: "board-map" })

    expect(layout.nodes).toEqual([])
    expect(layout.edges).toEqual([])
    expect(Number.isFinite(layout.width)).toBe(true)
    expect(Number.isFinite(layout.height)).toBe(true)
    expect(layout.width).toBeGreaterThan(0)
    expect(layout.height).toBeGreaterThan(0)
  })

  it("uses fallback layout for tiny graphs", async () => {
    const tinyGraph: TaskGraph = {
      nodes: [
        { id: "parent", ref: "default#1", title: "Parent", status: "ready" },
        { id: "child", ref: "default#2", title: "Child", status: "todo" },
      ],
      edges: [{ id: "dep:parent:child", sourceTaskId: "parent", targetTaskId: "child", kind: "dependency", blocking: true }],
    }

    expect(shouldUseFallbackLayout(tinyGraph)).toBe(true)
    await expect(layoutTaskGraphWithElk(tinyGraph, { mode: "board-map" })).resolves.toEqual(
      layoutTaskGraphFallback(tinyGraph, { mode: "board-map" }),
    )
  })

  it("uses graph topology instead of status buckets for board-map layers", async () => {
    const reverseLayout = await layoutTaskGraphWithElk(
      {
        nodes: [
          { id: "ready", ref: "default#1", title: "Ready", status: "ready", role: "context" },
          { id: "done", ref: "default#2", title: "Done context", status: "done", role: "context", contextOnly: true },
        ],
        edges: [{ id: "dep:done:ready", sourceTaskId: "done", targetTaskId: "ready", kind: "dependency", blocking: false }],
      },
      { mode: "board-map" },
    )
    const source = reverseLayout.nodes.find((node) => node.id === "done")
    const target = reverseLayout.nodes.find((node) => node.id === "ready")

    expect(source).toBeTruthy()
    expect(target).toBeTruthy()
    expect(source?.x).toBeLessThan(target?.x ?? 0)
  })

  it("keeps the fallback layout topology-directed", () => {
    const sameColumnLayout = layoutTaskGraphFallback(
      {
        nodes: [
          { id: "first", ref: "default#1", title: "First", status: "ready", role: "context" },
          { id: "second", ref: "default#2", title: "Second", status: "ready", role: "context" },
        ],
        edges: [{ id: "dep:first:second", sourceTaskId: "first", targetTaskId: "second", kind: "dependency", blocking: false }],
      },
      { mode: "board-map" },
    )
    const source = sameColumnLayout.nodes.find((node) => node.id === "first")
    const target = sameColumnLayout.nodes.find((node) => node.id === "second")

    expect(source).toBeTruthy()
    expect(target).toBeTruthy()
    expect(source?.x).toBeLessThan(target?.x ?? 0)
  })
})
