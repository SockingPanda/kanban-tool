import { readFileSync } from "node:fs"
import { describe, expect, it } from "vitest"

describe("desktop shell state boundaries", () => {
  it("keeps AppShell on grouped props instead of a flat controller surface", () => {
    const shellSource = readFileSync(new URL("./AppShell.tsx", import.meta.url), "utf8")

    expect(shellSource).toContain("runtime: AppShellRuntimeProps")
    expect(shellSource).toContain("navigation: AppShellNavigationProps")
    expect(shellSource).toContain("taskCollection: AppShellTaskCollectionProps")
    expect(shellSource).toContain("taskDetail: AppShellTaskDetailProps")
    expect(shellSource).toContain("taskCreation: AppShellTaskCreationProps")
    expect(shellSource).toContain("commands: AppShellCommandProps")
    expect(shellSource).toContain("export function AppShell({ runtime, navigation, taskCollection, taskDetail, taskCreation, commands }")
  })

  it("routes App controller state through focused hooks before composing shell props", () => {
    const appSource = readFileSync(new URL("../App.tsx", import.meta.url), "utf8")

    expect(appSource).toContain("useRuntimeConfigState()")
    expect(appSource).toContain("useTaskCollectionState(")
    expect(appSource).toContain("useSelectedTaskDetailState(")
    expect(appSource).toContain("useTaskCreationDialogState()")
    expect(appSource).toContain("useTaskMutations(")
    expect(appSource).toContain("<AppShell")
    expect(appSource).toContain("runtime={runtime}")
    expect(appSource).toContain("navigation={navigation}")
    expect(appSource).toContain("taskCollection={taskCollection}")
    expect(appSource).toContain("taskDetail={taskDetail}")
    expect(appSource).toContain("taskCreation={taskCreation}")
    expect(appSource).toContain("commands={commands}")
  })

  it("keeps shell regions behind memoized sub-boundaries", () => {
    const shellSource = readFileSync(new URL("./AppShell.tsx", import.meta.url), "utf8")

    expect(shellSource).toContain("const MemoShellSidebar = memo(ShellSidebar)")
    expect(shellSource).toContain("const MemoShellHeader = memo(ShellHeader)")
    expect(shellSource).toContain("const MemoMainView = memo(MainView)")
    expect(shellSource).toContain("const MemoTaskDetailSheet = memo(TaskDetailSheet)")
    expect(shellSource).toContain("const MemoStatusBar = memo(StatusBar)")
    expect(shellSource).toContain("<MemoShellSidebar")
    expect(shellSource).toContain("<MemoShellHeader")
    expect(shellSource).toContain("<MemoMainView")
    expect(shellSource).toContain("<MemoTaskDetailSheet")
    expect(shellSource).toContain("<MemoStatusBar")
  })

  it("keeps App command callbacks stable before composing the command group", () => {
    const appSource = readFileSync(new URL("../App.tsx", import.meta.url), "utf8")

    expect(appSource).toContain("const changeBoard = useCallback")
    expect(appSource).toContain("const closeTaskDetail = useCallback")
    expect(appSource).toContain("const refreshTasks = useCallback")
    expect(appSource).toContain("const resetListFilters = useCallback")
    expect(appSource).toContain("const setRowsPerPageCommand = useCallback")
    expect(appSource).toContain("changeBoard,")
    expect(appSource).toContain("resetListFilters,")
    expect(appSource).not.toContain("taskCollectionState,")
    expect(appSource).not.toContain("taskDetailState,")
  })
})
