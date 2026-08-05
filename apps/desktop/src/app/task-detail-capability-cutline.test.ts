import { readFileSync } from "node:fs"
import { describe, expect, it } from "vitest"

describe("task detail capability surface", () => {
  it("loads neighborhood data and keeps label suggestions manual", () => {
    const source = readFileSync(new URL("./useSelectedTaskDetailState.ts", import.meta.url), "utf8")

    expect(source).toContain("neighborhoodEnabled: enabled")
    expect(source).toContain("requestTaskLabelSuggestions")
    expect(source).toContain("useQuery")
    expect(source).toContain("queryKeys.taskLabelSuggestions")
    expect(source).toContain("enabled: false")
  })

  it("renders neighborhood, labels, label mutation, and edit controls", () => {
    const taskDetailSource = readFileSync(new URL("../features/task-detail/TaskDetail.tsx", import.meta.url), "utf8")
    const headerSource = readFileSync(new URL("../features/task-detail/TaskSummaryHeader.tsx", import.meta.url), "utf8")

    expect(taskDetailSource).toContain("One-hop map")
    expect(taskDetailSource).toContain("<TaskLabelsPanel")
    expect(taskDetailSource).toContain("api.addTaskLabel")
    expect(taskDetailSource).toContain("api.removeTaskLabel")
    expect(headerSource).toContain("onEdit")
    expect(headerSource).toContain("Pencil")
    expect(headerSource).toContain('t("Edit")')
  })
})
