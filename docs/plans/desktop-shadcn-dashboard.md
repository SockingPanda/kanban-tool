# Desktop Shadcn Dashboard Direction

**Goal:** Preserve the reviewed shadcn dashboard direction for future `apps/desktop` work without treating this task as a concrete component migration. The desktop app should evolve as a restrained, dense local operator console for the existing Kanban / durable work queue, not as a SaaS collaboration product.

**Current baseline:** `apps/desktop/components.json` already configures shadcn with the `new-york` style, Tailwind CSS variables, neutral base color, `@/components/ui` aliases, and `lucide` icons. The current UI is a React/Tauri operator shell with `AppShell`, `App.tsx`, board/list/detail/events/runs/maintenance/health/settings views, and only these local primitives: `button`, `input`, `textarea`, `badge`, and `separator`.

## Product Constraints

- Keep SQLite-only, localhost, single-user semantics. Do not introduce SaaS, organization, team, invite, RBAC, or multi-user assumptions in UI copy, IA, or state.
- Treat `tasks.status` as canonical truth. `board_columns` remains UI display mapping.
- Web/desktop actions must call the API/core command service path and respect the state machine. Drag/drop and action controls must not imply direct `tasks.status` mutation.
- `ready -> running` remains an atomic claim transaction owned by the backend. UI can display and initiate claim/start, but must not model it as ordinary column reassignment.
- `blocked -> ready` must be recomputed by service behavior. UI affordances should say unblock/recompute-ready semantics, not blind status set.
- `review` is human inspection state. Do not design dispatcher controls that claim or auto-run `review` tasks.

## Dashboard Direction

Use the shadcn dashboard as a reference for structure and density, not a source to copy wholesale.

- Use a grouped left sidebar with clear product identity, primary task surfaces, and separate system/operations pages.
- Keep a compact top header with global search, refresh/actions, task creation entry, active board/runtime indicators, and status controls.
- Prefer dense cards, counters, tabs, segmented controls, data tables, column controls, pagination, dialogs/sheets, tooltips, skeletons, and empty states where they clarify operator workflows.
- Keep the visual language quiet: neutral surfaces, clear borders, compact spacing, and low ornamentation. Avoid marketing-style heroes, decorative cards, and collaboration/SaaS language.
- Use `lucide-react` icons for navigation and icon buttons. Text labels should clarify commands, not describe the product.
- Continue using shadcn-compatible primitives and tokens so local components can converge incrementally instead of being replaced in one broad rewrite.

## Information Architecture

Split the desktop into two conceptual groups:

- **TaskExplorer:** task-focused working surfaces: Board, List, Task Detail, Runs for the selected task, and Event timeline for task/board activity.
- **System pages:** operator and environment surfaces: Maintenance, Health, Settings, database/runtime status, dispatcher visibility, and diagnostics.

`AppShell` should become the stable layout boundary:

- Own sidebar grouping, global header/search/actions, shell-level status indicators, and responsive main/detail regions.
- Delegate task data rendering to feature views. Board/List/Event/Run/detail components should not reimplement shell layout.
- Keep task creation and global filtering in predictable shell/header locations unless a feature view has a stronger local affordance.
- Preserve desktop ergonomics first; responsive behavior should avoid hidden state changes, overlapping controls, or oversized mobile-first composition.

## UI Primitive And Token Convergence

Future UI changes should converge toward reusable shadcn-style primitives in small slices:

- Add primitives only when a view actually needs them: `tabs`, `dialog` or `sheet`, `tooltip`, `skeleton`, `select`, `checkbox`, `table`, `dropdown-menu`, and pagination controls are likely candidates.
- Keep components under `apps/desktop/src/components/ui` compatible with the existing `components.json` aliases and token strategy.
- Route repeated variants through tokens and component variants rather than ad hoc per-view color classes.
- Preserve semantic status styling for task states. Do not collapse status colors into a one-note palette or purely decorative accents.
- Prefer compact controls with stable dimensions for toolbar buttons, filters, counters, cards, and table cells.

## Follow-Up Sequence

Implement visual/frontend follow-up in this order so architecture and behavior stay reviewable:

1. **Layout shell boundary first.** Refactor `AppShell` into a clearer shell/container boundary with grouped sidebar navigation and top header actions while preserving existing view behavior.
2. **UI primitives and theme.** Introduce the smallest needed shadcn-compatible primitives and token cleanup before touching data-heavy views.
3. **Events/poller latest/tail.** Tighten the Events view and poller semantics around latest events, tailing, refresh state, skeletons, empty states, and pagination/offset language.
4. **Board per-column queries vs List global pagination.** Decide and implement the data loading split explicitly: Board should support per-column status queries or clear per-column limits; List should own global pagination and search result semantics.
5. **TaskDetail/action cleanup.** Move dense task lifecycle controls into clearer sections, dialogs/sheets where needed, and state-machine-aware action affordances.
6. **Operator polish.** Add final dashboard polish: metrics, column controls, command/menu affordances, table density, loading/error states, keyboard-safe focus, and screenshot QA.

## View-Specific Notes

- **Board:** keep columns tied to status display mapping; any column controls must make hidden/display state distinct from canonical status.
- **List:** use data-table patterns for sortable/scannable task work. Pagination and search metadata should remain visible and deterministic.
- **Events:** design for audit/debug use, with latest/tail behavior and stale/degraded search or event stream states made explicit.
- **Runs:** show worker/run state as local execution history, not remote agent management.
- **Ontology Review:** keep it an operator review workbench over existing HTTP APIs. It may list unresolved signals, show grouped review queues, inspect signal/action details, and explain atoms. Lifecycle buttons should call the generic ontology action API only for review states such as confirm/reject/resolve-no-change; canonical mutation, validation, revert, or direct SQLite writes stay outside this view unless a later task explicitly adds a policy-gated flow.
- **Maintenance/Health/Settings:** keep them system/operator pages. Avoid user/admin/team language.
- **Task Detail:** actions should be grouped by legal lifecycle transitions and expose claim token requirements or failures only when relevant to the operator.

## Validation Expectations

For documentation-only changes, `git diff --check` is sufficient unless the edit affects frontend implementation.

For future `apps/desktop` implementation changes, run:

```bash
pnpm --dir apps/desktop test
pnpm --dir apps/desktop typecheck
git diff --check
```

Browser-only desktop development defaults to the local Vite proxy base
`/__kb_api__`, with `VITE_KB_DEV_PROXY_TARGET` defaulting to
`http://127.0.0.1:8721`. Start `kanban serve` on that origin before opening the
Vite page. Set `VITE_KB_API_BASE_URL` to a different API origin or proxy base
when targeting another runtime. The Vite dev server must not silently proxy
`/api` or `/health` to a hardcoded stale runtime.

When visual implementation changes shell/layout/components, also run browser or screenshot checks at representative desktop and narrow widths. Confirm there are no overlapping controls, blank views, clipped labels, or broken loading/empty states before claiming the UI work complete.

## Non-Goals

- Do not copy the full shadcn dashboard implementation into this repo.
- Do not perform a broad frontend rewrite only to match a reference screenshot.
- Do not introduce auth, users, organizations, teams, roles, SaaS billing, cloud sync, or remote worker concepts.
- Do not change API, data model, state machine, dispatcher behavior, or SQLite schema as part of a visual direction task.
- Do not add new dependencies unless a concrete implementation slice requires them and the existing stack cannot reasonably provide the behavior.
