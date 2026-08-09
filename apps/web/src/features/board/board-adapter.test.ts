import { describe, expect, test } from "vitest"

import type { BoardReadModel } from "../../lib/api/board-read-model"
import { asCanonicalBoardId } from "../../lib/sync/contracts"
import { toBoardViewModel } from "./board-adapter"

const readModel: BoardReadModel = {
  identity: {
    selector: "default",
    canonicalBoardId: asCanonicalBoardId("b_default"),
    slug: "default",
    name: "Default",
  },
  columns: [
    {
      id: "c_ready",
      board_id: "b_default",
      status: "ready",
      title: "Ready",
      position: 20,
      hidden: false,
      wip_limit: null,
      created_at: 1,
      updated_at: 2,
    },
    {
      id: "c_done",
      board_id: "b_default",
      status: "done",
      title: "Done",
      position: 30,
      hidden: true,
      wip_limit: 5,
      created_at: 1,
      updated_at: 2,
    },
  ],
  tasksByStatus: {
    ready: [
      {
        id: "t_ready",
        seq: 1,
        ref: "default#1",
        title: "Ship board",
        description: null,
        status: "ready",
        priority: 3,
        position: 10,
        lock_version: 7,
        assignee: "agent",
        dependency_blocked: true,
        unfinished_parent_count: 2,
        execution_plan_state: "planned",
        required_step_count: 3,
        completed_required_step_count: 1,
        optional_step_count: 1,
      },
    ],
    done: [],
  },
}

describe("BoardReadModel adapter", () => {
  test("maps every board identity, column, status group, task, and readiness field", () => {
    const model = toBoardViewModel(readModel)

    expect(model).toEqual({
      board: { id: "b_default", slug: "default", name: "Default" },
      columns: [
        { id: "c_ready", status: "ready", title: "Ready", position: 20, hidden: false },
        { id: "c_done", status: "done", title: "Done", position: 30, hidden: true },
      ],
      tasksByStatus: {
        ready: [
          {
            id: "t_ready",
            seq: 1,
            ref: "default#1",
            title: "Ship board",
            description: null,
            status: "ready",
            position: 10,
            priority: 3,
            assignee: "agent",
            lockVersion: 7,
            readiness: {
              dependencyBlocked: true,
              unfinishedParentCount: 2,
              executionPlanState: "planned",
              requiredStepCount: 3,
              completedRequiredStepCount: 1,
              optionalStepCount: 1,
            },
          },
        ],
        done: [],
      },
    })
  })

  test("returns an immutable presentation snapshot without wire-only fields", () => {
    const model = toBoardViewModel(readModel)

    expect(Object.isFrozen(model)).toBe(true)
    expect(Object.isFrozen(model.board)).toBe(true)
    expect(Object.isFrozen(model.columns)).toBe(true)
    expect(Object.isFrozen(model.columns[0])).toBe(true)
    expect(Object.isFrozen(model.tasksByStatus)).toBe(true)
    expect(Object.isFrozen(model.tasksByStatus.ready)).toBe(true)
    expect(Object.isFrozen(model.tasksByStatus.ready?.[0])).toBe(true)
    expect(model.columns[0]).not.toHaveProperty("board_id")
    expect(model.tasksByStatus.ready?.[0]).not.toHaveProperty("dependency_blocked")
  })
})
