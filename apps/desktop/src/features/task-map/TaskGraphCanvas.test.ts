import { describe, expect, it } from "vitest"

import { __test } from "./TaskGraphCanvas"
import type { TaskGraph } from "./task-graph-types"

describe("TaskGraphCanvas interactions", () => {
  it("shows zoom controls and viewport pan in detail mode", () => {
    const interaction = __test.taskGraphInteraction("detail")

    expect(interaction.showControls).toBe(true)
    expect(interaction.showMiniMap).toBe(false)
    expect(interaction.panOnDrag).toBe(true)
    expect(interaction.zoomOnPinch).toBe(true)
    expect(interaction.zoomOnScroll).toBe(true)
    expect(interaction.zoomOnDoubleClick).toBe(false)
    expect(interaction.maxZoom).toBeGreaterThan(1.75)
  })

  it("keeps board map controls and map interactions enabled", () => {
    const interaction = __test.taskGraphInteraction("board-map")

    expect(interaction.showControls).toBe(true)
    expect(interaction.showMiniMap).toBe(true)
    expect(interaction.panOnDrag).toBe(true)
    expect(interaction.zoomOnScroll).toBe(true)
    expect(interaction.zoomOnPinch).toBe(true)
    expect(interaction.zoomOnDoubleClick).toBe(true)
    expect(interaction.maxZoom).toBe(1.75)
  })

  it("keeps the layout key stable for non-topology task data", () => {
    const graph = graphFixture()
    const renamedGraph: TaskGraph = {
      ...graph,
      nodes: graph.nodes.map((node) => (node.id === "parent" ? { ...node, title: "Renamed parent", status: "blocked" } : node)),
    }
    const changedTopologyGraph: TaskGraph = {
      ...graph,
      edges: [...graph.edges, { id: "dep:other:child", sourceTaskId: "other", targetTaskId: "child", kind: "dependency" }],
    }

    expect(__test.taskGraphLayoutKey(renamedGraph, "board-map")).toBe(__test.taskGraphLayoutKey(graph, "board-map"))
    expect(__test.taskGraphLayoutKey(changedTopologyGraph, "board-map")).not.toBe(__test.taskGraphLayoutKey(graph, "board-map"))
  })

  it("uses blocked minimap color for dependency blocked nodes", () => {
    const color = __test.miniMapNodeColor({
      data: { node: { id: "child", ref: "#2", title: "Child", status: "ready", dependencyBlocked: true } },
    } as never)

    expect(color).toBe("#dc2626")
  })

  it("tracks task card data changes separately from topology layout", () => {
    const graph = graphFixture()
    const updatedGraph: TaskGraph = {
      ...graph,
      nodes: graph.nodes.map((node) => (node.id === "parent" ? { ...node, title: "Updated title", status: "blocked", stepCounts: { completed: 1, total: 3 } } : node)),
    }

    expect(__test.taskGraphLayoutKey(updatedGraph, "detail")).toBe(__test.taskGraphLayoutKey(graph, "detail"))
    expect(__test.taskGraphDataKey(updatedGraph)).not.toBe(__test.taskGraphDataKey(graph))
  })

  it("prefers explicit graph meta for layout keys when available", () => {
    const graph: TaskGraph = { ...graphFixture(), meta: { contentHash: "abc123" } }
    const updatedGraph: TaskGraph = {
      ...graph,
      nodes: graph.nodes.map((node) => ({ ...node, title: `${node.title} updated` })),
    }

    expect(__test.taskGraphMetaLayoutKey(graph)).toBe("hash:abc123")
    expect(__test.taskGraphLayoutKey(updatedGraph, "board-map")).toBe("board-map|hash:abc123")
  })

  it("stores graph layouts in a bounded LRU cache", () => {
    const layout = __test.layoutTaskGraphFallback(graphFixture(), { mode: "board-map" })

    __test.setCachedTaskGraphLayout("test-layout", layout)

    expect(__test.getCachedTaskGraphLayout("test-layout")).toBe(layout)
  })

  it("patches only previous and next selected flow nodes", () => {
    const nodes = __test.buildTaskFlowNodes(__test.layoutTaskGraphFallback(graphFixture(), { mode: "board-map" }), graphFixture(), "parent", undefined)
    const patched = __test.patchTaskFlowNodeSelection(nodes, "parent", "child")
    const originalById = new Map(nodes.map((node) => [node.id, node]))
    const patchedById = new Map(patched.map((node) => [node.id, node]))

    expect(patched).not.toBe(nodes)
    expect(patchedById.get("parent")).not.toBe(originalById.get("parent"))
    expect(patchedById.get("child")).not.toBe(originalById.get("child"))
    expect(patchedById.get("other")).toBe(originalById.get("other"))
    expect(patchedById.get("parent")?.selected).toBe(false)
    expect(patchedById.get("child")?.selected).toBe(true)
  })
})

function graphFixture(): TaskGraph {
  return {
    nodes: [
      { id: "parent", ref: "#1", title: "Parent", status: "todo", role: "context" },
      { id: "child", ref: "#2", title: "Child", status: "ready", role: "context" },
      { id: "other", ref: "#3", title: "Other", status: "running", role: "context" },
    ],
    edges: [{ id: "dep:parent:child", sourceTaskId: "parent", targetTaskId: "child", kind: "dependency", blocking: true }],
  }
}
