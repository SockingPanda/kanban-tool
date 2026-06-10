# V1 Local Web API Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Complete V1 as a localhost Web API release on top of the spec-compliant V0.5 CLI/SQLite command service.

**Architecture:** Add a `kanban-server` crate and `kanban serve` entrypoint that expose `/health` and `/api/v1` JSON endpoints. HTTP handlers must call the same `kanban-sqlite` service functions used by CLI; no route may mutate `tasks.status` directly. V1 is API-first and intentionally does not include a browser Web UI bundle.

**Tech Stack:** Rust workspace, SQLite-only via `kanban-sqlite`, Axum/Tokio HTTP server, serde JSON, integration tests with a temporary SQLite database and localhost router/service tests.

---

## V1 Scope

V1 includes:

- `kanban-server` crate.
- `kanban serve` command, default bind `127.0.0.1:8721`.
- Health endpoint: `GET /health`.
- Board API minimum: list/show default board and columns.
- Task API core lifecycle:
  - list/create/get/update
  - transitions: promote, claim, heartbeat, complete, submit-review, block, unblock, archive
  - dependency add/remove/list
  - list events and list runs
- SSE or equivalent event stream for `GET /api/v1/stream/events`.
- JSON success/error envelopes compatible with `docs/API_SPEC.md`.
- `docs/V1.md` release notes and README index update.

V1 does not include:

- Browser Web UI / drag-drop board frontend.
- Remote auth, users, RBAC, teams, tenants.
- HTTP backup/export endpoints.
- Attachments/labels/comments unless already needed for API scaffolding; comments may remain documented future work if not implemented.
- Persistent dispatcher service supervision; `kanban serve --dispatcher` may remain out of scope unless explicitly implemented.

## Gates

- **Pre-flight gate:** start from clean `main`, read AGENTS.md and API/DATA/STATE/CLI/DISPATCHER docs.
- **Revision gate per task:** implementer commit → spec reviewer PASS → quality reviewer APPROVED. P0/P1 loops back to a fix worker on the same branch.
- **Pre-merge gate:** parent runs `cargo fmt --check`, `cargo check --workspace --exclude kanban-desktop --tests`, `cargo nextest run --workspace --exclude kanban-desktop --no-fail-fast`, `cargo clippy --workspace --all-targets --exclude kanban-desktop -- -D warnings`, and a real `kanban serve` smoke with HTTP requests against a temp DB.
- **Final release gate:** independent final spec reviewer compares against `docs/API_SPEC.md` and `docs/V1.md`; independent quality reviewer checks server safety, transaction invariants, and route tests.

## Task 1: Server crate and health/board read APIs

**Objective:** Add `kanban-server` crate with router construction and basic read-only endpoints.

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/kanban-server/Cargo.toml`
- Create: `crates/kanban-server/src/lib.rs`
- Create: `crates/kanban-server/src/main.rs` if useful
- Test: `cargo nextest run -p kanban-server --no-fail-fast`

**TDD steps:**
1. Add failing integration tests for `GET /health`, `GET /api/v1/boards`, `GET /api/v1/boards/default`, and `GET /api/v1/boards/default/columns` using a temp DB.
2. Verify RED.
3. Implement app state, JSON envelope helpers, board/column query service functions if missing, and router.
4. Verify GREEN and commit.

**Review checklist:**
- Defaults bind/health do not require remote network.
- JSON shape is `{ "data": ... }` with optional `meta`.
- No handler writes status directly.

## Task 2: Task CRUD HTTP API

**Objective:** Expose V0.5 task CRUD through HTTP while preserving service invariants.

**Files:**
- Modify: `crates/kanban-server/src/lib.rs` or route modules
- Modify: `crates/kanban-sqlite/src/service.rs` only for missing query helpers
- Test: `cargo nextest run -p kanban-server --no-fail-fast tasks`

**TDD steps:**
1. Add failing tests for:
   - `POST /api/v1/boards/default/tasks` creates task and event.
   - `GET /api/v1/boards/default/tasks` lists non-archived tasks.
   - `GET /api/v1/tasks/{id}` returns task.
   - `PATCH /api/v1/tasks/{id}` updates editable fields and rejects `status` in body.
   - Update with future `scheduled_at` recomputes status as V0.5 does.
2. Verify RED.
3. Implement DTOs and route handlers calling `kanban-sqlite` service.
4. Verify GREEN and commit.

**Review checklist:**
- `status`, claim fields, and completed fields cannot be patched.
- Actor priority: body actor, `X-KB-Actor`, default actor.
- Errors use stable JSON envelope.

## Task 3: Transition, dependencies, runs, and events APIs

**Objective:** Expose core lifecycle endpoints and read APIs required by Web UI.

**Files:**
- Modify: server routes
- Modify: sqlite service only for missing helpers
- Test: `cargo nextest run -p kanban-server --no-fail-fast transitions`

**TDD steps:**
1. Add failing tests for transitions:
   - claim returns claim token/run and creates running task.
   - heartbeat extends claim.
   - complete moves running to done and promotes child.
   - submit-review moves running to review and dispatcher/claim does not execute review.
   - block/unblock recomputes target.
   - archive hides task from default list.
2. Add failing tests for dependencies add/remove/list.
3. Add failing tests for `GET /api/v1/events?board=default&after=0&limit=...` and `GET /api/v1/tasks/{id}/runs`.
4. Verify RED.
5. Implement handlers using service functions only.
6. Verify GREEN and commit.

**Review checklist:**
- Transitions do not patch status directly.
- Token mismatch maps to proper HTTP error.
- Dependency cycle maps to 409/400 with JSON error.

## Task 4: Serve CLI, SSE stream, and V1 docs

**Objective:** Make the server runnable via CLI and document V1 accurately.

**Files:**
- Modify: `crates/kanban-cli/src/main.rs`
- Modify: `crates/kanban-cli/Cargo.toml`
- Modify/Create: server crate files
- Create: `docs/V1.md`
- Modify: `README.md`
- Test: CLI/server smoke tests if practical

**TDD steps:**
1. Add failing test or smoke script for `kanban serve --help` and router SSE stream behavior.
2. Implement `kanban serve --host 127.0.0.1 --port 0/8721 --db <path>` if feasible; default must be localhost.
3. Implement `GET /api/v1/stream/events` as a simple SSE stream that emits existing events after `after` and can close for tests, or document exact V1 limitation if a continuous stream is deferred.
4. Add `docs/V1.md` with implemented scope, out-of-scope items, verification commands, and smoke recipe.
5. Verify and commit.

**Review checklist:**
- Server never binds non-localhost by default.
- SSE event IDs are monotonic DB event ids.
- Docs do not overclaim browser UI or unsupported endpoints.

## Final Integration Review

Dispatch two fresh reviewers:

1. **Spec reviewer:** compare implementation against `AGENTS.md`, `docs/API_SPEC.md`, `docs/STATE_MACHINE.md`, `docs/DATA_MODEL.md`, `docs/ARCHITECTURE.md`, and `docs/V1.md`. Return PASS only if no P0/P1 remains.
2. **Quality reviewer:** review HTTP safety, error handling, service reuse, test adequacy, transaction invariants, and localhost security. Return APPROVED only if no P0/P1 remains.

Only after both approve may the parent squash merge to `main`, delete the branch, rerun verification, and run a real HTTP smoke.
