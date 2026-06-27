# Task Map, Detail Workbench, and Step Execution Plan

This document records the implemented public contract for the public step-based execution-plan model.

## Product Terms

| Concept | Meaning | Storage |
|---|---|---|
| Description | Markdown specification: background, goal, constraints, and acceptance notes. | `tasks.description` |
| Step | Ordered execution-plan item inside a parent task. It can be plain text or link to another task for context. | `task_steps` |
| Dependency | Hard blocking relationship where parent completion gates child readiness. | `task_dependencies` |
| Task map | Operational graph of task-to-task relationships. Pure text steps do not appear as graph nodes. | read model over tasks/dependencies/steps |

Steps and dependencies are separate. A linked step does not create a dependency,
and the linked task status does not automatically resolve the step. If a linked
task must also block or unlock another task, create an explicit dependency edge.

## Execution Plan State

```text
unplanned     no steps and no explicit not_required plan
planned       at least one step exists
not_required  no steps, explicit reason recorded
```

A task cannot be promoted, claimed, or dispatcher-claimed while its plan is
`unplanned`. Marking a plan `not_required` requires a non-empty reason and is
rejected once the task has any steps.

Required steps block parent completion and archive until every required step is
`done` or `skipped`. Optional steps never block parent completion.

## Step Semantics

A step has:

- `id`, `parent_task_id`, `position`, `title`, and optional `body`;
- optional `linked_task_id` for context;
- `required` flag;
- independent `status`: `todo`, `done`, or `skipped`;
- resolution metadata: `resolution_note`, `resolved_by`, `resolved_at`.

Graph read models include `kind=step` edges only when a step has a linked task
and both the parent task and linked task are visible. Plain text steps remain in
step APIs and Detail execution-plan UI, not in the task graph.

## API Surface

```http
GET    /api/v1/tasks/{task_id}/steps
POST   /api/v1/tasks/{task_id}/steps
PATCH  /api/v1/tasks/{task_id}/steps/{step_id}
DELETE /api/v1/tasks/{task_id}/steps/{step_id}
POST   /api/v1/tasks/{task_id}/steps/{step_id}/done
POST   /api/v1/tasks/{task_id}/steps/{step_id}/skip
POST   /api/v1/tasks/{task_id}/steps/{step_id}/reopen
POST   /api/v1/tasks/{task_id}/execution-plan/not-required
```

Task DTOs expose `execution_plan_state`, `required_step_count`,
`completed_required_step_count`, and `optional_step_count`.

Graph endpoints keep the same shapes but use step terminology:

```http
GET /api/v1/tasks/{task_id}/neighborhood?depth=1
GET /api/v1/boards/{board}/task-map?context_depth=1
```

`TaskGraphEdge.kind` is `dependency` or `step`. Node roles include
`step_parent` and `step_child` for linked steps.

## CLI Surface

```bash
kanban task step list <task_ref>
kanban task step add <task_ref> <title> [--body <text>] [--link-task <task_ref>] [--required|--optional]
kanban task step update <task_ref> <step_ref> [--title <text>] [--body <text>|--clear-body] [--link-task <task_ref>|--unlink-task] [--required|--optional]
kanban task step done <task_ref> <step_ref> --note <text>
kanban task step skip <task_ref> <step_ref> --reason <text>
kanban task step reopen <task_ref> <step_ref> --reason <text>
kanban task step remove <task_ref> <step_ref>
kanban task step not-required <task_ref> --reason <text>
```

`kanban task list` supports `--plan-needed`, `--has-steps`,
`--incomplete-required-steps`, and repeated `--plan-filter` with the same filter
names.

## Detail and Map UI

Task Detail is the current-task workbench: description, execution plan steps,
primary action, discussion, runs/events, metadata, and the one-hop task graph.
The graph shows dependencies and linked-step task context; the steps section is
where plain text steps and independent step status live.

The Map page is the board-level operational graph. It shows active tasks plus
one-hop context and `dependency`/`step` edges between visible task nodes. It is
not an infinite graph explorer and it does not render plain text steps as nodes.
