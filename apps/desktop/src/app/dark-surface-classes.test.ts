import { readFileSync } from "node:fs"
import { fileURLToPath } from "node:url"
import { describe, expect, it } from "vitest"

const sourceRoot = fileURLToPath(new URL("../", import.meta.url))

const targetedViewFiles = [
  "features/board/BoardView.tsx",
  "features/list/ListView.tsx",
  "features/runs/RunsView.tsx",
  "features/events/EventsView.tsx",
  "features/health/HealthView.tsx",
  "features/maintenance/MaintenanceView.tsx",
  "features/settings/SettingsView.tsx",
  "features/task-detail/TaskDetail.tsx",
]

const neutralTextLevels = ["900", "800", "700", "600", "500", "400", "300", "200", "100", "50"]
const bannedLightOnlyClasses = [
  ["bg", "white"].join("-"),
  ["bg", "neutral", "50"].join("-"),
  "bg-" + "[#f7f7f5]",
  ["border", "neutral", "200"].join("-"),
  ["text", "red", "700"].join("-"),
  ...neutralTextLevels.map((level) => ["text", "neutral", level].join("-")),
]
const bannedLightOnlyClassPattern = new RegExp(
  `\\b(?:${bannedLightOnlyClasses.map((className) => escapeRegExp(className)).join("|")})\\b`,
  "g",
)

describe("desktop dark surface coverage", () => {
  it("keeps targeted feature views on semantic surface tokens", () => {
    const violations = targetedViewFiles.flatMap((relativePath) => {
      const content = readFileSync(new URL(relativePath, `file://${sourceRoot}`), "utf8")
      return [...content.matchAll(bannedLightOnlyClassPattern)].map((match) => `${relativePath}: ${match[0]}`)
    })

    expect(violations).toEqual([])
  })
})

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")
}
