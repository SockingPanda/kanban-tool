import { describe, expect, test } from "vitest"

import {
  parseTaskSelector,
  queryForTasksView,
  type TasksViewQuery,
} from "./tasks-url"

describe("Tasks URL task selector contract", () => {
  test("distinguishes a missing selector, a canonical selector, and malformed input", () => {
    expect(parseTaskSelector("")).toEqual({ kind: "missing", value: null })
    expect(parseTaskSelector("task=t_ready")).toEqual({ kind: "valid", value: "t_ready" })

    expect(parseTaskSelector("task=")).toMatchObject({ kind: "malformed", value: "" })
    expect(parseTaskSelector("task=../escape")).toMatchObject({ kind: "malformed", value: "../escape" })
    expect(parseTaskSelector("task=t_ready%20")).toMatchObject({ kind: "malformed", value: "t_ready " })
    expect(parseTaskSelector("task=t_ready&task=t_other")).toMatchObject({ kind: "malformed" })
  })
})

describe("Tasks URL query ownership", () => {
  const source = "q=agent&task=t_ready&status=ready&priority=2&plan=has_steps&sort=-updated_at&page=2&limit=25&include_archived=true&display=table&filter=blocked&show_done=true&hide_isolated=true&zoom=1.3&unknown=drop"

  test.each([
    ["board", "q=agent&task=t_ready&status=ready"],
    ["list", "q=agent&task=t_ready&status=ready&priority=2&plan=has_steps&sort=-updated_at&page=2&limit=25&include_archived=true&display=table"],
    ["table", "q=agent&task=t_ready&status=ready&priority=2&plan=has_steps&sort=-updated_at&page=2&limit=25&include_archived=true&display=table"],
    ["map", "q=agent&task=t_ready&filter=blocked&show_done=true&hide_isolated=true&zoom=1.3"],
  ] satisfies readonly [TasksViewQuery, string][]) ("keeps only %s-owned query fields", (view, expected) => {
    expect(queryForTasksView(source, view).toString()).toBe(expected)
  })

  test("switching to grouped List removes table display while retaining only shared and List state", () => {
    expect(queryForTasksView(source, "list", "grouped").toString()).toBe("q=agent&task=t_ready&status=ready&priority=2&plan=has_steps&sort=-updated_at&page=2&limit=25&include_archived=true")
  })

  test("retains malformed task text as shared URL state for an explicit UI error", () => {
    const query = queryForTasksView("q=agent&task=../escape&status=ready&filter=blocked", "map")
    expect(query.toString()).toBe("q=agent&task=../escape&filter=blocked")
    expect(parseTaskSelector(query)).toMatchObject({ kind: "malformed", value: "../escape" })
  })
})
