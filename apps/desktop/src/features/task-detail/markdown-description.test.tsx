import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, it } from "vitest"

import type { Task } from "@/lib/api"
import { Sheet } from "@/components/ui/sheet"

import { emptyDetail } from "./detail-state"
import { MarkdownDescription, TaskDetail } from "./TaskDetail"

describe("MarkdownDescription", () => {
  it("renders GFM markdown without enabling raw HTML", () => {
    const html = renderToStaticMarkup(
      <MarkdownDescription>{"**bold**\n\n- item\n\n<script>alert('x')</script>"}</MarkdownDescription>,
    )

    expect(html).toContain("<strong>bold</strong>")
    expect(html).toContain("<li>item</li>")
    expect(html).not.toContain("<script>")
    expect(html).toContain("&lt;script&gt;")
  })

  it("renders links as external links and filters unsafe protocols", () => {
    const html = renderToStaticMarkup(
      <MarkdownDescription>{"[safe](https://example.com) [bad](javascript:alert('x'))"}</MarkdownDescription>,
    )

    expect(html).toContain('href="https://example.com"')
    expect(html).toContain('target="_blank"')
    expect(html).toContain('rel="noreferrer noopener"')
    expect(html).not.toContain('href="javascript:alert')
  })

  it("renders task comments with the same safe markdown rules", () => {
    const html = renderToStaticMarkup(
      <Sheet open>
        <TaskDetail
          api={null}
          task={task}
          detail={{
            ...emptyDetail,
            comments: [
              {
                id: "c_1",
                board_id: task.board_id,
                task_id: task.id,
                author: "codex",
                author_type: "agent",
                agent_type: "codex",
                body: "**bold**\n\n- item\n\n<script>alert('x')</script>\n\n[bad](javascript:alert('x'))",
                kind: "text",
                created_at: task.created_at,
              },
            ],
          }}
          blockReason=""
          setBlockReason={() => undefined}
          dependencyInput=""
          setDependencyInput={() => undefined}
          claimToken={null}
          commentBody=""
          setCommentBody={() => undefined}
          editDraft={null}
          draftDirty={false}
          setEditDraft={() => undefined}
          detailLoading={false}
          pendingAction={null}
          onAction={async () => undefined}
          onAddDependency={async () => undefined}
          onRemoveDependency={async () => undefined}
          onSelectTask={() => undefined}
          onSaveTask={async () => true}
          onCancelEdit={() => undefined}
          onAddComment={async () => undefined}
        />
      </Sheet>,
    )

    expect(html).toContain("<strong>bold</strong>")
    expect(html).toContain("<li>item</li>")
    expect(html).not.toContain("<script>")
    expect(html).toContain("&lt;script&gt;")
    expect(html).not.toContain('href="javascript:alert')
  })
})

const task: Task = {
  id: "t_1",
  board_id: "b_1",
  board_slug: "kanban-tool",
  ref: "kanban-tool#1",
  seq: 1,
  title: "Render comment markdown",
  description: null,
  status: "ready",
  status_reason: null,
  assignee: null,
  priority: 1,
  position: 0,
  scheduled_at: null,
  due_at: null,
  created_by: "codex",
  created_at: 1_781_441_329_826,
  updated_at: 1_781_441_329_826,
  started_at: null,
  completed_at: null,
  archived_at: null,
  claim_owner: null,
  claim_expires_at: null,
  last_heartbeat_at: null,
  current_run_id: null,
  retry_count: 0,
  max_retries: null,
  result_summary: null,
  result_json: null,
  metadata_json: "{}",
  lock_version: 0,
  dependency_blocked: false,
  unfinished_parent_count: 0,
}
