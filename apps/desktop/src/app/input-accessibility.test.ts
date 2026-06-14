import { readFileSync } from "node:fs"
import { fileURLToPath } from "node:url"
import { describe, expect, it } from "vitest"

const sourceRoot = fileURLToPath(new URL("../", import.meta.url))

function source(relativePath: string) {
  return readFileSync(new URL(relativePath, `file://${sourceRoot}`), "utf8")
}

describe("desktop input accessibility contracts", () => {
  it("keeps shared controls keyboard-visible", () => {
    expect(source("components/ui/button.tsx")).toContain("focus-visible:ring-2")
    expect(source("components/ui/input.tsx")).toContain("focus-visible:ring-2")
    expect(source("components/ui/textarea.tsx")).toContain("focus-visible:ring-2")
  })

  it("names shell and task form fields without browser autocomplete noise", () => {
    const appShell = source("app/AppShell.tsx")
    const taskDetail = source("features/task-detail/TaskDetail.tsx")

    expect(appShell).toContain('aria-label="Search tasks"')
    expect(appShell).toContain('name="task-search"')
    expect(appShell).toContain('autoComplete="off"')
    expect(appShell).toContain('name="new-task-title"')
    expect(appShell).toContain('name="new-task-description"')

    for (const name of [
      "task-title",
      "task-description",
      "task-assignee",
      "task-scheduled-at",
      "task-due-at",
      "block-reason",
      "parent-task-id",
      "comment-body",
    ]) {
      expect(taskDetail).toContain(`name="${name}"`)
    }
  })

  it("announces async errors and uses typographic ellipsis in touched UI", () => {
    const appShell = source("app/AppShell.tsx")
    const taskDetail = source("features/task-detail/TaskDetail.tsx")

    expect(appShell).toContain('role="alert"')
    expect(appShell).toContain('aria-live="assertive"')
    expect(taskDetail).toContain('"Saving…"')
    expect(stringLiterals(`${appShell}\n${taskDetail}`).filter((literal) => literal.includes("..."))).toEqual([])
  })
})

function stringLiterals(content: string) {
  return [...content.matchAll(/(["'`])((?:\\.|(?!\1).)*)\1/gs)].map((match) => match[2])
}
