import { readFileSync } from "node:fs"
import { fileURLToPath } from "node:url"
import { dirname, resolve } from "node:path"

import { describe, expect, it } from "vitest"

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..")

function source(path: string) {
  return readFileSync(resolve(root, path), "utf8")
}

describe("desktop shadcn control convergence", () => {
  it("keeps scoped toolbar, list, and detail controls off native select inputs", () => {
    const files = [
      "app/AppShell.tsx",
      "features/list/ListView.tsx",
      "features/task-detail/TaskDetail.tsx",
    ]

    for (const file of files) {
      const content = source(file)
      expect(content, file).not.toContain("NativeSelect")
      expect(content, file).not.toContain("<select")
      expect(content, file).not.toContain('type="checkbox"')
    }
  })
})
