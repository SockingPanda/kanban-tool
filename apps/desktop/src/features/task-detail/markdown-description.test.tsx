import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, it } from "vitest"

import type { Task, TaskStep, TaskSteps } from "@/lib/api"
import { Sheet } from "@/components/ui/sheet"

import { emptyDetail } from "./detail-state"
import { __test as markdownTest } from "./markdown"
import { MarkdownDescription, TaskDetail } from "./TaskDetail"

describe("MarkdownDescription", () => {
  it("uses stable markdown renderer configuration", () => {
    const initialRemarkPlugins = markdownTest.markdownRemarkPlugins
    const initialComponents = markdownTest.markdownComponents

    renderToStaticMarkup(<MarkdownDescription>{"[safe](https://example.com)"}</MarkdownDescription>)
    renderToStaticMarkup(<MarkdownDescription>{"| a |\n| - |\n| b |"}</MarkdownDescription>)

    expect(markdownTest.markdownRemarkPlugins).toBe(initialRemarkPlugins)
    expect(markdownTest.markdownComponents).toBe(initialComponents)
  })

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
                metadata: {},
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
          commentsExpanded
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
                metadata: {
                  options: [
                    { slug: "metadata", title: "Use metadata", detail: "**Keep** it in comment metadata." },
                    { slug: "table", title: "Add table", detail: "Create separate decision rows." },
                  ],
                  selected: "metadata",
                  reason: "Keeps the choice next to the discussion.",
                  risk: "Schema drift.",
                  verification: "Render and service tests.",
                },
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
          commentsExpanded
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
                metadata: {
                  options: [{ slug: " metadata ", title: "Metadata", detail: "Invalid raw slug." }],
                  selected: "metadata",
                  reason: "Whitespace-padded slugs must not be accepted.",
                },
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
          commentsExpanded
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

  it("renders complete signal backlink metadata while preserving the readable body fallback", () => {
    const html = renderToStaticMarkup(
      <Sheet open>
        <TaskDetail
          api={null}
          task={task}
          detail={{
            ...emptyDetail,
            comments: [
              {
                id: "c_signal",
                board_id: task.board_id,
                task_id: task.id,
                author: "codex",
                author_type: "agent",
                agent_type: "codex",
                body: "Signal captured for operator review.",
                kind: "signal",
                metadata: {
                  type: "signal_link",
                  signal_id: "sig_fixture",
                  observation_id: "obs_fixture",
                  signal_kind: "agent_cli_failure",
                  signal_status: "open",
                },
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
          commentsExpanded
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

    expect(html).toContain("Signal captured for operator review.")
    expect(html).toContain("sig_fixture")
    expect(html).toContain("agent_cli_failure")
    expect(html).toContain("open")
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
              metadata: {},
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
          commentsExpanded
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
    expect(html).toContain("touches Rust service code")
    expect(html).toContain("add API handler")
    expect(html).toContain("negative evidence 1")
  })

  it("renders dependencies, execution plan, and gated primary action", () => {
    const parentTask: Task = { ...task, id: "t_parent", ref: "default#2", seq: 2, title: "Parent blocker", status: "blocked" }
    const childTask: Task = { ...task, id: "t_child", ref: "default#3", seq: 3, title: "Unlocked child", status: "todo" }
    const html = renderToStaticMarkup(
      <Sheet open>
        <TaskDetail
          api={null}
          task={task}
          detail={{
            ...emptyDetail,
            dependencies: { parents: [parentTask], children: [childTask] },
            neighborhood: {
              center_task_id: task.id,
              nodes: [
                { task, role: "center", context_only: false },
                { task: parentTask, role: "dependency_parent", context_only: false },
                { task: childTask, role: "dependency_child", context_only: false },
              ],
              edges: [
                { id: "e_parent", source_task_id: parentTask.id, target_task_id: task.id, kind: "dependency", required: true, blocking: true },
                { id: "e_child", source_task_id: task.id, target_task_id: childTask.id, kind: "dependency", required: true, blocking: false },
              ],
              meta: { generated_at: task.created_at, truncated: false, node_count: 3, edge_count: 2, depth: 1 },
            },
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
          dependenciesExpanded
          graphExpanded
          stepsExpanded
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

    expect(html).toContain("One-hop map")
    expect(html).toContain("Parent blocker")
    expect(html).toContain("Unlocked child")
    expect(html).toContain("Execution plan")
    expect(html).toContain("Execution plan is not planned")
    expect(html).toContain("Primary action")
    expect(html).toContain("Plan steps first")
    expect(html).toContain("More actions")
    expect(html).not.toContain("Legal transitions")
  })

  it("renders resolved execution plan rows without required or done badges", () => {
    const taskWithSteps: Task = {
      ...task,
      execution_plan_state: "planned",
      required_step_count: 3,
      completed_required_step_count: 2,
    }
    const html = renderToStaticMarkup(
      <Sheet open>
        <TaskDetail
          api={null}
          task={taskWithSteps}
          detail={{
            ...emptyDetail,
            steps: stepsFixture(taskWithSteps.id),
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
          stepsExpanded
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

    expect(html).toContain("2/3 steps")
    expect(html).toContain("Step finished")
    expect(html).toContain("border-lime-300 bg-lime-50")
    expect(html).toContain("Step skipped")
    expect(html).toContain("bg-muted/30 text-muted-foreground")
    expect(html).not.toMatch(/<span[^>]*>required<\/span>/)
    expect(html).not.toMatch(/<span[^>]*>done<\/span>/)
  })

})

const task: Task = {
  id: "t_1",
  board_id: "b_1",
  board_slug: "kanban-tool",
  ref: "default#1",
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
  result: null,
  metadata: {},
  lock_version: 0,
  dependency_blocked: false,
  unfinished_parent_count: 0,
  execution_plan_state: "unplanned",
  required_step_count: 0,
  completed_required_step_count: 0,
  optional_step_count: 0,
  labels: [],
}

function stepsFixture(taskId: string): TaskSteps {
  return {
    task_id: taskId,
    execution_plan: {
      board_id: "b_1",
      task_id: taskId,
      state: "planned",
      reason: null,
      updated_by: "codex",
      updated_at: task.updated_at,
    },
    steps: [
      stepFixture(taskId, "step_done", "Step finished", "done"),
      stepFixture(taskId, "step_skipped", "Step skipped", "skipped"),
      stepFixture(taskId, "step_todo", "Step pending", "todo"),
    ],
  }
}

function stepFixture(parentTaskId: string, id: string, title: string, status: TaskStep["status"]): TaskStep {
  return {
    id,
    parent_task_id: parentTaskId,
    title,
    body: null,
    linked_task: null,
    position: 1024,
    required: true,
    status,
    resolution_note: null,
    resolved_by: null,
    resolved_at: null,
    created_by: "codex",
    created_at: task.created_at,
    updated_by: "codex",
    updated_at: task.updated_at,
  }
}
