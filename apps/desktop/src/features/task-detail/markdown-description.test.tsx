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
                kind: "note",
                metadata_json: "{}",
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

  it("renders decision comments with selected option and structured fields", () => {
    const html = renderToStaticMarkup(
      <Sheet open>
        <TaskDetail
          api={null}
          task={task}
          detail={{
            ...emptyDetail,
            comments: [
              {
                id: "c_decision",
                board_id: task.board_id,
                task_id: task.id,
                author: "codex",
                author_type: "agent",
                agent_type: "codex",
                body: "Choose where to store decisions.",
                kind: "decision",
                metadata_json: JSON.stringify({
                  options: [
                    { slug: "metadata", title: "Use metadata", detail: "**Keep** it in comment metadata." },
                    { slug: "table", title: "Add table", detail: "Create separate decision rows." },
                  ],
                  selected: "metadata",
                  reason: "Keeps the choice next to the discussion.",
                  risk: "Schema drift.",
                  verification: "Render and service tests.",
                }),
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

    expect(html).toContain("decision")
    expect(html).toContain("metadata")
    expect(html).toContain("table")
    expect(html).toContain("Use metadata")
    expect(html).toContain("<strong>Keep</strong>")
    expect(html).toContain("Keeps the choice next to the discussion.")
    expect(html).toContain("Schema drift.")
    expect(html).toContain("Render and service tests.")
    expect(html.split("bg-[var(--status-ready-bg)]").length - 1).toBeGreaterThanOrEqual(2)
  })

  it("shows invalid decision metadata as an alert while keeping body fallback", () => {
    const html = renderToStaticMarkup(
      <Sheet open>
        <TaskDetail
          api={null}
          task={task}
          detail={{
            ...emptyDetail,
            comments: [
              {
                id: "c_bad_decision",
                board_id: task.board_id,
                task_id: task.id,
                author: "codex",
                author_type: "agent",
                agent_type: "codex",
                body: "Fallback decision body.",
                kind: "decision",
                metadata_json: JSON.stringify({
                  options: [{ slug: " metadata ", title: "Metadata", detail: "Invalid raw slug." }],
                  selected: "metadata",
                  reason: "Whitespace-padded slugs must not be accepted.",
                }),
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

    expect(html).toContain("Fallback decision body.")
    expect(html).toContain('role="alert"')
    expect(html).toContain("Invalid decision metadata")
    expect(html).toContain("option slug must be lowercase ASCII letters, digits, or hyphen")
  })

  it("renders the first newest-first comment page with pagination controls", () => {
    const html = renderToStaticMarkup(
      <Sheet open>
        <TaskDetail
          api={null}
          task={task}
          detail={{
            ...emptyDetail,
            comments: Array.from({ length: 12 }, (_, index) => ({
              id: `c_${index + 1}`,
              board_id: task.board_id,
              task_id: task.id,
              author: "codex",
              author_type: "agent",
              agent_type: "codex",
              body: `Body ${String(index + 1).padStart(2, "0")}`,
              kind: "note",
              metadata_json: "{}",
              created_at: task.created_at + index,
            })),
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

    expect(html.indexOf("Body 12")).toBeLessThan(html.indexOf("Body 03"))
    expect(html).toContain("Page 1 of 2")
    expect(html).toContain("Newest first")
    expect(html).toContain('aria-label="Next comments"')
    expect(html).not.toContain("Body 02")
    expect(html).not.toContain("Body 01")
  })

  it("renders task labels and label input controls", () => {
    const html = renderToStaticMarkup(
      <Sheet open>
        <TaskDetail
          api={null}
          task={{
            ...task,
            labels: [
              { id: "l_backend", board_id: task.board_id, name: "backend", color: null, created_at: 1, updated_at: 1 },
            ],
          }}
          detail={emptyDetail}
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

    expect(html).toContain("Labels")
    expect(html).toContain("backend")
    expect(html).toContain('aria-label="Label name"')
    expect(html).toContain('aria-label="Add label"')
    expect(html).toContain('aria-label="Remove label backend"')
    expect(html).toContain("Suggest labels")
    expect(html).not.toContain("Suggestions unavailable")
    expect(html).not.toContain("No label suggestions.")
  })

  it("renders degraded label suggestions and already applied state", () => {
    const html = renderToStaticMarkup(
      <Sheet open>
        <TaskDetail
          api={null}
          task={{
            ...task,
            labels: [
              { id: "l_backend", board_id: task.board_id, name: "backend", color: null, created_at: 1, updated_at: 1 },
            ],
          }}
          detail={emptyDetail}
          labelSuggestions={{
            task_id: task.id,
            board_id: task.board_id,
            selected_labels: [
              {
                label_id: "l_backend",
                label_name: "backend",
                score: 0.82,
                weight: 0.82,
                already_applied: true,
                evidence_atoms: [
                  {
                    atom_id: "la_positive",
                    label_id: "l_backend",
                    label_name: "backend",
                    polarity: "positive",
                    kind: "applies_when",
                    text: "touches Rust service code",
                    score: 0.81,
                  },
                  {
                    atom_id: "la_example",
                    label_id: "l_backend",
                    label_name: "backend",
                    polarity: "positive",
                    kind: "positive_example",
                    text: "add API handler",
                    score: 0.73,
                  },
                ],
                negative_evidence_atoms: [
                  {
                    atom_id: "la_negative",
                    label_id: "l_backend",
                    label_name: "backend",
                    polarity: "negative",
                    kind: "excludes_when",
                    text: "CSS-only",
                    score: 0.42,
                  },
                ],
              },
            ],
            candidates: [],
            coverage: 0.82,
            coverage_cosine: 0.91,
            residual_norm: 0.18,
            needs_new_label: false,
            reason_codes: ["degraded_result", "label_atom_index_dirty"],
            degraded: true,
            diagnostics: ["label_atom_index_dirty"],
          }}
          labelSuggestionsRequested
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

    expect(html).toContain("Suggestions")
    expect(html).toContain("Degraded")
    expect(html).toContain("label_atom_index_dirty")
    expect(html).toContain("Applied")
    expect(html).toContain("coverage 82%")
    expect(html).toContain("cosine 91%")
    expect(html).toContain("residual 0.180")
    expect(html).not.toContain("new label may be needed")
    expect(html).toContain("touches Rust service code")
    expect(html).toContain("add API handler")
    expect(html).toContain("negative evidence 1")
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
  execution_plan_state: "unplanned",
  required_subtask_count: 0,
  completed_required_subtask_count: 0,
  optional_subtask_count: 0,
  labels: [],
}
