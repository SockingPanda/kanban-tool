import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test, vi } from "vitest"

import type { BoardTaskMutationClient, BoardTaskMutationSurface } from "../../application/tasks/task-mutation-state"
import { TaskBoard } from "./task-board"
import { type BoardViewModel } from "../../domain/tasks/board"

const model: BoardViewModel = {
  board: { id: "b_default", slug: "default", name: "Default" },
  columns: [
    { id: "c_todo", status: "todo", title: "Todo", position: 1, hidden: false },
    { id: "c_ready", status: "ready", title: "Ready", position: 2, hidden: false },
    { id: "c_running", status: "running", title: "Running", position: 3, hidden: false },
  ],
  tasksByStatus: {
    todo: [{
      id: "t_1",
      seq: 1,
      ref: "default#1",
      title: "Draft",
      description: "Draft specification",
      status: "todo",
      position: 1,
      scheduledAt: null,
      dueAt: null,
      lastHeartbeatAt: null,
      statusReason: null,
      labels: [],
      lockVersion: 3,
      priority: 2,
      assignee: null,
      readiness: {
        dependencyBlocked: false,
        unfinishedParentCount: 0,
        executionPlanState: "not_required",
        requiredStepCount: 0,
        completedRequiredStepCount: 0,
        optionalStepCount: 0,
      },
    }],
    ready: [],
    running: [],
  },
}

function surface(): BoardTaskMutationSurface {
  const client: BoardTaskMutationClient = {
    createTask: vi.fn(),
    createStep: vi.fn(),
    updateTask: vi.fn(),
    transitionTask: vi.fn(),
  }
  return { client, onCanonicalReload: vi.fn() }
}

describe('TaskBoard', () => {
  test('使用四个展示列，保留每张卡片的真实状态', () => {
    const markup = renderToStaticMarkup(<TaskBoard model={model} mutations={surface()} visibleIds={['t_1']} onSelectTask={vi.fn()} />);
    for (const label of ['待开始','进行中','待验收','已完成']) expect(markup).toContain('aria-label="'+label+'"');
    expect(markup).toContain('data-status="todo"');
    expect(markup).toContain('draggable="true"');
    expect(markup).toContain('aria-keyshortcuts="Space Escape ArrowLeft ArrowRight Enter"');
    expect(markup).not.toContain('style=');
  });
  test('只展示当前查询页的任务', () => {
    const markup = renderToStaticMarkup(<TaskBoard model={model} mutations={surface()} visibleIds={[]} onSelectTask={vi.fn()} />);
    expect(markup).not.toContain('data-task-id="t_1"');
  });
});
