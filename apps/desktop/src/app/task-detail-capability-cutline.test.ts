import { readFileSync } from "node:fs"
import { describe, expect, it } from "vitest"

describe("task detail capability cutline", () => {
  it("disables unsupported detail requests and keeps legacy suggestions inert", () => {
    const source = readFileSync(new URL("./useSelectedTaskDetailState.ts", import.meta.url), "utf8")

    expect(source).toContain("neighborhoodEnabled: false")
    expect(source).not.toContain("requestTaskLabelSuggestions")
    expect(source).not.toContain("useQuery")
    expect(source).not.toContain("queryKeys.taskLabelSuggestions")
  })

  it("does not render neighborhood, labels, label mutation, or edit controls", () => {
    const taskDetailSource = readFileSync(new URL("../features/task-detail/TaskDetail.tsx", import.meta.url), "utf8")
    const headerSource = readFileSync(new URL("../features/task-detail/TaskSummaryHeader.tsx", import.meta.url), "utf8")

    expect(taskDetailSource).not.toContain("One-hop map")
    expect(taskDetailSource).not.toContain("<TaskLabelsPanel")
    expect(taskDetailSource).not.toContain("api.addTaskLabel")
    expect(taskDetailSource).not.toContain("api.removeTaskLabel")
    expect(headerSource).not.toContain("onEdit")
    expect(headerSource).not.toContain("Pencil")
    expect(headerSource).not.toContain('t("Edit")')
  })
})
