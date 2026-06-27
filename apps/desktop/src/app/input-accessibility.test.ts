import { readdirSync, readFileSync } from "node:fs"
import { fileURLToPath } from "node:url"
import { describe, expect, it } from "vitest"

const sourceRoot = fileURLToPath(new URL("../", import.meta.url))

function source(relativePath: string) {
  return readFileSync(new URL(relativePath, `file://${sourceRoot}`), "utf8")
}

function taskDetailSource() {
  const directory = new URL("features/task-detail/", `file://${sourceRoot}`)

  return readdirSync(directory)
    .filter((name) => name.endsWith(".tsx"))
    .sort()
    .map((name) => readFileSync(new URL(name, directory), "utf8"))
    .join("\n")
}

describe("desktop input accessibility contracts", () => {
  it("keeps shared controls keyboard-visible", () => {
    expect(source("components/ui/button.tsx")).toContain("focus-visible:ring-2")
    expect(source("components/ui/input.tsx")).toContain("focus-visible:ring-2")
    expect(source("components/ui/textarea.tsx")).toContain("focus-visible:ring-2")
  })

  it("names shell and Add task dialog fields without browser autocomplete noise", () => {
    const appShell = source("app/AppShell.tsx")
    const taskDetail = taskDetailSource()
    const addTaskDialog = appShell.slice(
      appShell.indexOf("function AddTaskDialog"),
      appShell.indexOf("function MainView"),
    )

    expect(appShell).toContain('aria-label="Search tasks"')
    expect(appShell).toContain('name="task-search"')
    expect(appShell).toContain('autoComplete="off"')
    expect(addTaskDialog).toContain('aria-label="Add task"')
    expect(addTaskDialog).toContain('aria-label="New task title"')
    expect(addTaskDialog).toContain('name="new-task-title"')
    expect(addTaskDialog).toContain('aria-label="New task description"')
    expect(addTaskDialog).toContain('name="new-task-description"')
    expect(addTaskDialog).toContain('autoComplete="off"')

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
    const taskDetail = taskDetailSource()

    expect(appShell).toContain('role="alert"')
    expect(appShell).toContain('aria-live="assertive"')
    expect(taskDetail).toContain('"Saving…"')
    expect(stringLiterals(`${appShell}\n${taskDetail}`).filter((literal) => literal.includes("..."))).toEqual([])
  })
})

function stringLiterals(content: string) {
  return [...content.matchAll(/(["'`])((?:\\.|(?!\1).)*)\1/gs)].map((match) => match[2])
}
