# Task Map and Execution Plan Contract

Status: frozen V1 contract for follow-up implementation tasks.

This document is the product and implementation contract for task graph views and first-class execution plans. Later tasks must reference this file instead of inventing local semantics in Desktop, API, CLI, dispatcher, or migration work.

## Product Terms

| Term | Meaning | Canonical storage |
|---|---|---|
| Description | Markdown spec text for why the task exists, desired result, scope, acceptance, and notes. It is not an execution checklist and not a dependency list. | `tasks.description` |
| Execution plan | First-class ordered subtasks/steps that describe how this task will be executed. A task can require a plan, be planned, or explicitly mark the plan as not required with a reason. | `task_execution_plans` plus `task_subtasks` |
| Subtask | A child work unit created to decompose a parent task. Subtasks express task breakdown and completion structure. They do not imply blocking order by themselves. | `task_subtasks` |
| Step | A task-backed ordered execution item inside a task plan. A required step must be represented by a first-class child task through `task_subtasks`; embedded checklist text or Markdown cannot satisfy the execution-plan gate. | `task_subtasks` |
| Dependency | A hard blocking edge where parent must be `done` or `archived` before child may become `ready` or `running`. | `task_dependencies` |
| Task Detail workbench | The current task workbench: spec, plan, comments, runs, dependencies, subtasks, and one-hop graph around the selected task. | Desktop Detail view |
| Board Map terrain | A global active board map for situational awareness across open work. | Desktop Map page |

Description, execution plan, subtasks, and dependencies are separate product concepts. UI may display them together, but writes must target the correct command/API surface and must not infer one concept from another.

## Data Model Contract

`tasks.description` remains spec text only. It continues to satisfy the existing "description/spec present" guard for `ready`, but it does not satisfy the execution-plan gate.

`task_dependencies` remains the hard blocking DAG:

- `parent_task_id -> child_task_id` means parent blocks child.
- Only parent statuses `done` and `archived` satisfy the hard dependency guard.
- Dependency changes continue to use dependency commands/endpoints and service transactions.
- Dependency edges participate in `dependency_blocked`, `unfinished_parent_count`, promote guard, claim guard, and dispatcher eligibility.
- Dependency edges are not subtasks and are not execution steps.

`task_subtasks` is the future first-class parent/child decomposition table:

- It links `parent_task_id -> child_task_id` on the same board.
- It stores display/order metadata for the parent's execution plan.
- It stores `required` as a boolean, defaults new subtask links to `required=true`, and only allows optional links through explicit service/API/CLI input.
- Required subtasks block parent completion until the child is `done` or `archived`; optional subtasks never block parent completion.
- It does not by itself block the child from running or the parent from running.
- It can coexist with a `task_dependencies` edge when a subtask must also be a hard prerequisite.
- It must reject self-links and cycles in the subtask tree/graph for the same board.
- It must be modified through service/API/CLI commands, not by direct table writes.

`task_execution_plans` is the canonical plan-state table for a parent task:

- It has exactly one row per task once the task is created, or service reads must derive an implicit `unplanned` row for legacy tasks until migration backfill completes.
- It stores `state`, `reason` for `not_required`, audit fields, and optional plan metadata.
- It does not store Markdown checklist items as executable work.
- Required executable steps are represented by task ids linked through `task_subtasks`.
- Only `planned` or `not_required` with a non-empty reason can satisfy executable readiness.

Execution plan state has these values:

| State | Meaning |
|---|---|
| `unplanned` | A required execution plan has not been supplied yet. |
| `planned` | The task has a sufficient plan: at least one active required subtask/step task linked through `task_subtasks`. |
| `not_required` | A plan is not required for this task, and `execution_plan_not_required_reason` is non-empty. |

The default for new executable tasks is `unplanned` unless creation explicitly marks `not_required` with a reason or creates a valid plan in the same service transaction.

## Graph DTO Contract

Graph APIs return a task graph DTO that is independent of the Desktop rendering library:

```json
{
  "data": {
    "scope": "detail",
    "center_task_id": "t_...",
    "nodes": [
      {
        "id": "t_...",
        "ref": "default#123",
        "title": "A freeze contract",
        "status": "running",
        "role": "center",
        "execution_plan_state": "planned",
        "dependency_blocked": false,
        "unfinished_parent_count": 0,
        "subtask_counts": { "total": 3, "incomplete": 1 }
      }
    ],
    "edges": [
      {
        "id": "dep:t_parent:t_child",
        "kind": "dependency",
        "source_task_id": "t_parent",
        "target_task_id": "t_child",
        "blocking": true
      },
      {
        "id": "subtask:t_parent:t_child",
        "kind": "subtask",
        "source_task_id": "t_parent",
        "target_task_id": "t_child",
        "blocking": false,
        "position": 1024
      }
    ],
    "meta": {
      "truncated": false,
      "node_count": 1,
      "edge_count": 0,
      "layout": "deterministic-v1"
    }
  }
}
```

Nodes use task ids as stable ids. Edge ids include edge kind and endpoint ids so dependency and subtask edges can coexist between the same two tasks. A `dependency` edge is directional parent -> child. A `subtask` edge is directional parent -> child but has `blocking=false` unless a matching dependency edge also exists.

DTOs do not expose pixel positions as canonical truth in V1. Layout is deterministic client or server projection from graph scope, status, edge kind, and task ordering.

## Detail Graph Scope

Task Detail graph is a one-hop workbench graph around the selected task.

The node set includes:

- the center task;
- direct dependency parents of the center task;
- direct dependency children of the center task;
- direct subtasks of the center task;
- the direct parent task if the center task is itself a subtask.

The edge set includes:

- every dependency edge where both endpoints are visible;
- every subtask edge where both endpoints are visible.

Detail graph must not recursively expand beyond one hop in V1. It should favor legibility and workbench actions over exploration breadth. It may show `done` direct context muted, because completed parents explain why the center task can proceed. It hides `archived` context by default unless the selected task itself is archived or the user explicitly enables archived context.

## Board Map Graph Scope

Board Map is the global board terrain for active work. It is not a task detail replacement.

The base node set includes all tasks on the selected board where:

```sql
status NOT IN ('done', 'archived')
```

The context node set adds one non-archived hop around those active tasks:

- non-archived dependency parents;
- non-archived dependency children;
- non-archived direct subtasks;
- non-archived direct subtask parents.

`done` context nodes may be included as muted explanation when they connect to active nodes. `archived` nodes are hidden by default. Board Map responses must include truncation metadata when node or edge caps are applied; V1 should use conservative caps rather than adding complex viewport persistence.

The edge set includes all dependency and subtask edges where both endpoints are visible.

## Execution Plan Gates

Execution plan is required before a task may enter the executable queue unless it is marked `not_required` with a non-empty reason.

Blocked transitions:

- create with initial `ready` when `execution_plan_state = unplanned`;
- `triage -> todo` / specify when the task becomes executable but remains `unplanned`;
- `todo -> ready` / promote when the task is `unplanned`;
- `scheduled -> ready` when schedule matures or a user promotes but the task is `unplanned`;
- `blocked -> ready` or any recomputation that would produce `ready` while the task is `unplanned`;
- `ready -> running` / claim when the task is `unplanned`;
- dispatcher claim when the task is `unplanned`.

The guard applies to every service path that enters or consumes `ready`, including promote, unblock/recompute, schedule maturation, manual claim, and dispatcher claim. UI controls may hide impossible actions, but backend service guards are authoritative.

Allowed transitions while `unplanned`:

- update description/spec text;
- edit execution plan fields, steps, and subtasks;
- add or remove dependency edges, subject to existing dependency rules;
- move to or remain in `triage`, `todo`, `scheduled`, or `blocked`;
- archive, restore, comment, attach, label, and other non-execution metadata operations that already bypass executable eligibility.

Parent completion is blocked while required subtasks are incomplete:

- A task with required subtasks cannot complete to `done` while any required direct subtask is not `done` or `archived`.
- Optional subtasks do not block parent completion.
- Force-complete, if implemented, must require an explicit local repair flag, write an event/comment-grade audit trail, and remain unavailable to dispatcher automation.

Dependency completion and subtask completion are separate checks. A child can be a subtask without being a dependency; in that case it blocks parent completion only if it is required, but it does not block parent claim. A child can be both subtask and dependency; then it blocks both child/parent graph semantics as specified by each edge type.

## Backend And API

Backend work must keep SQLite as the only canonical database and must use the existing service path shared by CLI, HTTP, Desktop, and dispatcher.

Required backend surfaces for later tasks:

- migrations for `task_execution_plans` and `task_subtasks`;
- service commands for plan update, subtask add/remove/reorder, required/optional updates, and step update;
- transition guards for promote/specify/claim/dispatcher claim and parent completion;
- API DTO fields on task responses for `execution_plan_state`, `execution_plan_not_required_reason`, required/optional subtask counts, and completion-blocking subtask counts;
- graph endpoints for Detail and Board Map scopes;
- events for plan and subtask mutations.

Suggested endpoints:

```http
GET /api/v1/tasks/{task_id}/graph?scope=detail
GET /api/v1/boards/{board_id_or_slug}/task-map
PUT /api/v1/tasks/{task_id}/execution-plan
POST /api/v1/tasks/{parent_task_id}/subtasks
DELETE /api/v1/tasks/{parent_task_id}/subtasks/{child_task_id}
PATCH /api/v1/tasks/{parent_task_id}/subtasks/{child_task_id}
```

Subtask create/attach defaults `required=true`. PATCH must allow changing `position` and `required`; setting `required=false` is an explicit optional-subtask operation and must write an event.

Exact endpoint names may change during API implementation, but the semantics in this document must not. Any alternative naming must preserve the DTO fields and graph scopes above.

## Desktop Component Boundaries

V1 must not introduce React Flow / XYFlow. The initial graph renderer is:

- lightweight HTML task cards;
- SVG edge overlay;
- deterministic layout;
- no persisted node positions;
- no edge editing;
- no minimap;
- no box select;
- no bulk graph actions.

React Flow / XYFlow can be reconsidered later only if observed needs include hundreds of nodes, persisted layout, complex pan/zoom/minimap, edge editing, box select, or bulk actions. The renderer choice is intentionally not part of the product semantics.

Desktop boundaries:

- `AppShell` remains the outer shell for sidebar, header, search, and global actions.
- Task Detail is a wide three-region workbench: left region is the Detail one-hop map/context graph; main region is description, execution plan/subtasks, primary actions, discussion, and collapsed runs/events; right region is metadata such as assignee, labels, dates, board, status, and ids.
- Board Map is a top-level view/page for global active board terrain.
- Board/List/Event/Run views do not duplicate shell layout.
- Graph actions call API/service commands; UI never writes `tasks.status`, `task_dependencies`, or future `task_subtasks` directly.

The UI must keep local-first, single-user, localhost operator console semantics. Do not add SaaS, teams, RBAC, invitations, cloud sync, or remote worker assumptions.

## Phased Implementation Order

1. Add backend data model and service tests for execution-plan state, `task_subtasks`, and parent completion guard.
2. Add CLI/API command coverage for plan state and subtask management.
3. Add graph query service and DTO tests for Detail one-hop scope.
4. Add graph query service and DTO tests for Board Map active terrain scope and truncation metadata.
5. Add Desktop data hooks and lightweight graph renderer with deterministic layout.
6. Integrate Detail graph into Task Detail workbench.
7. Add Board Map page as global terrain.
8. Add focused docs/help updates and update later task descriptions to reference this contract.

Each phase must preserve existing dependency semantics and status-machine invariants.

## Tests, Docs, And Skill Sync Gates

Test gates for implementation tasks:

- service tests for plan gate blocking promote/claim/dispatcher claim;
- service tests for parent completion blocked by incomplete required subtasks;
- service tests proving optional subtasks do not block parent completion;
- service tests proving subtask edge alone does not create dependency blocking;
- API/CLI tests for plan state, subtask CRUD, required defaulting, and required/optional mutation;
- graph DTO tests for Detail node/edge scope;
- graph DTO tests for Board Map active + one-hop context scope;
- Desktop typecheck and component tests for deterministic graph rendering;
- screenshot/browser checks only when visual implementation changes actual UI.

Documentation gates:

- update `docs/API_SPEC.md`, `docs/CLI_SPEC.md`, `docs/DATA_MODEL.md`, `docs/STATE_MACHINE.md`, and `docs/DISPATCHER_SPEC.md` when implementation changes those contracts;
- update task descriptions for later graph tasks if they still assume React Flow/XYFlow in V1;
- check the global `kanban-tool` skill after any user-visible CLI/API/workflow behavior is implemented.

This #285 documentation-only contract does not change shipped CLI/API behavior, so global skill sync is not required for this task.

## Non-Goals

- No React Flow / XYFlow dependency in V1.
- No new frontend dependency for the initial graph renderer.
- No SaaS, multi-user collaboration, RBAC, teams, invitations, cloud sync, or remote worker model.
- No direct status writes from Desktop.
- No conversion of `task_dependencies` into a general knowledge graph.
- No persisted freeform graph layout in V1.
- No automatic promotion of children when dependencies complete.
- No dispatcher claim of `review` or unplanned tasks.
