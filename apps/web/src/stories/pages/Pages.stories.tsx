import { useState, type ReactNode } from "react"
import type { Meta, StoryObj } from "@storybook/react-vite"
import { userEvent, within } from "storybook/test"

import type { BoardListItem } from "../../lib/api/board-list-read-model"
import type { ExplorerTaskMap, ExplorerTaskMapReadModel, TaskListQueryState } from "../../lib/api/explorer-read-model"
import type { BoardEventsReadModel, ExplorerEvent } from "../../lib/api/events-read-model"
import type { HealthReport } from "../../lib/api/health-read-model"
import type {
  DoctorReport,
  MaintenanceApi,
  MaintenanceStatus,
  QueueStats,
  SearchStatus,
} from "../../lib/api/maintenance-api"
import type {
  LabelAtomExplainRecord,
  LabelOntologyReviewGroup,
  LabelOntologySignalDetail,
  LabelOntologySignalRecord,
  SignalRecord,
} from "../../lib/api/signals-ontology-read-model"
import { assertCanonicalBoardSlug } from "../../lib/board-slug"
import { PreferencesProvider } from "../../lib/preferences-provider"
import type { WebRuntimeConfig } from "../../lib/runtime"
import { asCanonicalBoardId } from "../../lib/sync/contracts"
import type { ReadState } from "../../features/read-state"
import { BoardView } from "../../features/board/BoardView"
import type { BoardTaskViewModel, BoardViewModel, BoardViewState } from "../../features/board/types"
import { EventsPresentation, type EventsReadState } from "../../features/explorer/EventsView"
import { TaskListView, type TaskListRow, type TaskListViewState } from "../../features/explorer/TaskListView"
import { TaskMapPresentation, type TaskMapReadState } from "../../features/explorer/TaskMapView"
import { TaskRunsPresentation, type TaskRunsReadState } from "../../features/explorer/TaskRunsView"
import { HealthPage } from "../../features/health/HealthPage"
import { MaintenancePage } from "../../features/maintenance/MaintenancePage"
import { OntologyScreenView } from "../../features/ontology/OntologyScreen"
import { ProjectOverview } from "../../features/projects/ProjectOverview"
import { ProjectsCollection } from "../../features/projects/ProjectsCollection"
import { SignalsScreenView } from "../../features/signals/SignalsScreen"
import { SettingsPage } from "../../features/settings/SettingsPage"
import type { OntologyRouteFilters, SignalsRouteFilters } from "../../lib/router"
import type { BoardTaskStatus } from "../../features/board/types"

/**
 * Production page owners rendered in isolation. Every fixture is local typed
 * data: this module never opens HTTP/SSE, calls a mutation, or uses a raster.
 */
const meta = {
  title: "Pages/Routes",
  parameters: {
    layout: "fullscreen",
    docs: {
      description: {
        component: "Canonical 12-route page evidence. Fixtures are typed read models only; no API, SSE, mutation, remote image, or raster asset is used.",
      },
    },
  },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

function PageStoryFrame({ children }: { readonly children: ReactNode }) {
  return (
    <PreferencesProvider>
      <main className="min-h-screen min-w-0 bg-body p-4 text-primary sm:p-6">
        {children}
      </main>
    </PreferencesProvider>
  )
}

const runtime = {
  apiBaseUrl: "",
  webBasePath: "/app/",
  actor: "storybook",
  defaultBoard: "default",
  serverVersion: "3.0.0",
  protocolVersion: "v1",
  webBuildId: "storybook-fixture",
} satisfies WebRuntimeConfig

const boardIdentity = {
  id: asCanonicalBoardId("b_default"),
  slug: assertCanonicalBoardSlug("default"),
  name: "Default",
} as const

const project = {
  id: boardIdentity.id,
  slug: boardIdentity.slug,
  name: "Default",
  description: "The local-first work queue for the product team.",
  archivedAt: null,
} satisfies BoardListItem

const projects: readonly BoardListItem[] = [
  project,
  {
    id: asCanonicalBoardId("b_docs"),
    slug: assertCanonicalBoardSlug("docs"),
    name: "Docs",
    description: "Documentation and release notes.",
    archivedAt: null,
  },
]

const taskReadiness = {
  dependencyBlocked: false,
  unfinishedParentCount: 0,
  executionPlanState: "planned" as const,
  requiredStepCount: 2,
  completedRequiredStepCount: 1,
  optionalStepCount: 0,
}

function boardTask(id: string, status: BoardTaskStatus, overrides: Partial<BoardTaskViewModel> = {}): BoardTaskViewModel {
  return {
    id,
    seq: Number(id.replace(/\D/g, "")) || 1,
    ref: `default#${id}`,
    title: status === "blocked" ? "Resolve the blocked dependency" : "Review the canonical task snapshot",
    description: "A typed Storybook read-model fixture.",
    status,
    position: 0,
    scheduledAt: null,
    dueAt: null,
    lastHeartbeatAt: null,
    statusReason: status === "blocked" ? "Waiting for an upstream task." : null,
    labels: [{ id: "label-ui", name: "ui", color: "#6b7280" }],
    lockVersion: 1,
    priority: status === "blocked" ? 2 : 1,
    assignee: status === "running" ? "storybook" : null,
    readiness: status === "blocked" ? { ...taskReadiness, dependencyBlocked: true } : taskReadiness,
    ...overrides,
  }
}

const boardModel: BoardViewModel = {
  board: boardIdentity,
  columns: [
    { id: "todo", status: "todo", title: "To do", position: 0, hidden: false },
    { id: "ready", status: "ready", title: "Ready", position: 1, hidden: false },
    { id: "running", status: "running", title: "Running", position: 2, hidden: false },
    { id: "review", status: "review", title: "Review", position: 3, hidden: false },
    { id: "done", status: "done", title: "Done", position: 4, hidden: false },
    { id: "blocked", status: "blocked", title: "Blocked", position: 5, hidden: false },
  ],
  tasksByStatus: {
    todo: [boardTask("t_101", "todo")],
    ready: [boardTask("t_102", "ready")],
    running: [boardTask("t_103", "running")],
    review: [],
    done: [],
    blocked: [boardTask("t_104", "blocked")],
  },
}

const boardReady: BoardViewState = { kind: "ready", model: boardModel }
const boardEmpty: BoardViewState = { kind: "empty", board: boardIdentity, detail: "No tasks have been added to this board yet." }

const listQuery: TaskListQueryState = {
  status: [],
  priority: [],
  plan: [],
  search: "",
  sort: "updated_at",
  page: 1,
  limit: 25,
  includeArchived: false,
}

const listRows: readonly TaskListRow[] = [
  {
    id: "t_101",
    ref: "default#101",
    title: "Review the canonical task snapshot",
    status: "ready",
    priority: 1,
    assignee: "storybook",
    executionPlanState: "planned",
    dependencyBlocked: false,
    requiredStepCount: 2,
    completedRequiredStepCount: 1,
    optionalStepCount: 0,
    updatedAt: 1_700_000_000,
  },
  {
    id: "t_104",
    ref: "default#104",
    title: "Resolve the blocked dependency",
    status: "blocked",
    priority: 2,
    assignee: null,
    executionPlanState: "unplanned",
    dependencyBlocked: true,
    requiredStepCount: 0,
    completedRequiredStepCount: 0,
    optionalStepCount: 0,
    updatedAt: 1_700_000_001,
  },
]

const listState: TaskListViewState = { query: listQuery, meta: { offset: 0, limit: 25, total: listRows.length } }
const updateListQuery = (_query: TaskListQueryState) => {
  void _query
}

type MapTask = ExplorerTaskMap["nodes"][number]["task"]
function mapTask(id: string, status: MapTask["status"], overrides: Partial<MapTask> = {}): MapTask {
  return {
    id,
    board_id: boardIdentity.id,
    board_slug: boardIdentity.slug,
    ref: `default#${id}`,
    seq: Number(id.replace(/\D/g, "")) || 1,
    title: id === "t_201" ? "Parent task" : "Child task",
    description: "Map node from the canonical explorer read model.",
    status,
    status_reason: null,
    assignee: null,
    priority: 1,
    position: 0,
    scheduled_at: null,
    due_at: null,
    created_by: "storybook",
    created_at: 1_700_000_000,
    updated_at: 1_700_000_000,
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
    execution_plan_state: "planned",
    required_step_count: 1,
    completed_required_step_count: 0,
    optional_step_count: 0,
    labels: [],
    ...overrides,
  }
}

const map: ExplorerTaskMap = {
  nodes: [
    { task: mapTask("t_201", "ready"), role: "active", context_only: false },
    { task: mapTask("t_202", "running"), role: "active", context_only: false },
    { task: mapTask("t_203", "done"), role: "context", context_only: true },
  ],
  edges: [
    { id: "dep:t_201:t_202", source_task_id: "t_201", target_task_id: "t_202", kind: "dependency", required: true, blocking: false },
    { id: "dep:t_203:t_201", source_task_id: "t_203", target_task_id: "t_201", kind: "dependency", required: false, blocking: false },
  ],
  meta: {
    depth: 1,
    context_depth: 1,
    generated_at: 1_700_000_000,
    node_count: 3,
    edge_count: 2,
    truncated: false,
    active_statuses: ["ready", "running"],
    active_only: true,
    include_done_context: true,
    include_archived_context: false,
    hide_isolated: false,
    limit_nodes: 240,
  },
}

const mapModel: ExplorerTaskMapReadModel = { board: { selector: "default", ...boardIdentity }, map }
const mapReady: TaskMapReadState = { data: mapModel, loading: false, error: null }
const mapLoading: TaskMapReadState = { data: null, loading: true, error: null }

function event(id: number, overrides: Partial<ExplorerEvent> = {}): ExplorerEvent {
  return {
    id,
    event_id: `event-${id}`,
    board_id: boardIdentity.id,
    task_id: "t_101",
    run_id: null,
    kind: id === 1 ? "task.updated" : "task.created",
    actor: "storybook",
    payload: { source: "fixture" },
    created_at: 1_700_000_000 + id,
    ...overrides,
  }
}

const eventsModel: BoardEventsReadModel = {
  board: { selector: "default", id: boardIdentity.id, slug: boardIdentity.slug, name: boardIdentity.name },
  taskId: null,
  events: [event(1), event(2)],
  meta: { count: 2, nextAfter: 2, limit: 150 },
}
const eventsReady: EventsReadState = { data: eventsModel, loading: false, error: null, stale: false }

const runsReady: TaskRunsReadState = {
  data: {
    taskId: "t_101",
    runs: [{
      id: "r_101",
      task_id: "t_101",
      status: "succeeded",
      worker_profile: "storybook",
      worker_pid: null,
      claim_owner: "storybook",
      started_at: 1_700_000_000,
      finished_at: 1_700_000_010,
      exit_code: 0,
      summary: "Fixture run completed.",
      error: null,
      has_log: true,
      metadata: {},
    }],
    selectedRunId: "r_101",
    log: { run_id: "r_101", content: "storybook fixture run completed", truncated: false },
  },
  loading: false,
  error: null,
}

function readState<T>(data: T, phase: ReadState<T>["phase"] = "success", error: unknown | null = null): ReadState<T> {
  return { phase, data, error }
}

const signal: SignalRecord = {
  id: "sig_101",
  board_id: boardIdentity.id,
  observation_id: "obs_101",
  kind: "agent_cli_friction",
  title: "CLI friction",
  summary: "A command was rejected by the local workflow.",
  severity: "info",
  status: "open",
  dedupe_key: "storybook-signal",
  superseded_by_signal_id: null,
  reviewed_by: null,
  reviewed_at: null,
  review_reason: null,
  created_at: 1_700_000_000,
  updated_at: 1_700_000_001,
  observation: {
    id: "obs_101",
    board_id: boardIdentity.id,
    task_id: "t_101",
    task_ref_snapshot: "default#101",
    run_id: null,
    comment_id: null,
    actor: "storybook",
    agent_type: "executor",
    source: "storybook-fixture",
    evidence: { command: "kanban task show", exit_code: 1 },
    created_at: 1_700_000_000,
  },
}

const signalFilters: SignalsRouteFilters = { status: "open", kinds: ["agent_cli_friction"], task: "default#101" }

const ontologySignal: LabelOntologySignalRecord = {
  id: "los_101",
  observation_id: "loo_101",
  board_id: boardIdentity.id,
  kind: "false_negative",
  status: "open",
  target_label_id: "lab_cli",
  target_label_name_snapshot: "cli",
  related_labels: [],
  proposed_action: "add_positive_atom",
  candidate_atom_polarity: "positive",
  candidate_atom_kind: "applies_when",
  candidate_text: "touches CLI behavior",
  candidate_content_hash: "hash_101",
  proposed_label_name: null,
  proposed_label_name_normalized: null,
  proposal: {},
  agent_selected: true,
  suggest_state: "absent",
  suggest_score: 0.12,
  suggest_rank: 1,
  final_selected: true,
  rationale: "Review rationale from a static fixture.",
  confidence: 0.9,
  signal_key: "storybook-ontology",
  superseded_by_signal_id: null,
  status_reason: null,
  created_at: 1_700_000_000,
  updated_at: 1_700_000_001,
  reviewed_at: null,
  closed_at: null,
}

const ontologyDetail: LabelOntologySignalDetail = {
  signal: ontologySignal,
  observation: {
    id: "loo_101",
    board_id: boardIdentity.id,
    task_id: "t_101",
    task_ref_snapshot: "default#101",
    task_snapshot: {},
    suggest_input_hash: "input-hash-101",
    agent_candidates: [],
    suggestion_snapshot: {},
    final_decision: {},
    suggest_coverage: 0.6,
    suggest_coverage_cosine: 0.7,
    suggest_residual_norm: 0.4,
    suggest_needs_new_label: false,
    suggest_degraded: false,
    diagnostics: [],
    capture_fingerprint: "fingerprint-101",
    created_by: "storybook",
    created_by_type: "user",
    agent_type: null,
    created_at: 1_700_000_000,
    signals: [],
  },
  actions: [],
}

const ontologyGroup: LabelOntologyReviewGroup = {
  group_by: "label",
  key: "lab_cli",
  label_id: "lab_cli",
  label_name: "cli",
  candidate_atom_polarity: "positive",
  candidate_atom_kind: "applies_when",
  candidate_text: "touches CLI behavior",
  candidate_content_hash: "hash_101",
  proposed_label_name: null,
  proposed_label_name_normalized: null,
  cluster_key: null,
  cluster_reason: null,
  task_count: 1,
  signal_count: 1,
  open_count: 1,
  confirmed_count: 0,
  resolved_count: 0,
  rejected_count: 0,
  superseded_count: 0,
  degraded_count: 0,
  average_score: 0.12,
  median_score: 0.12,
  oldest_signal_at: 1_700_000_000,
  latest_signal_at: 1_700_000_001,
  sample_task_refs: ["default#101"],
  signal_ids: [ontologySignal.id],
  action_count: 0,
  action_ids: [],
  proposal_ids: [],
  labels: [{ id: "lab_cli", name: "cli" }],
  candidate_atom_variants: [],
}

const atom: LabelAtomExplainRecord = {
  query: "hash_101",
  atom: {
    id: "lat_101",
    label_id: "lab_cli",
    board_id: boardIdentity.id,
    label_name: "cli",
    polarity: "positive",
    kind: "applies_when",
    text: "touches CLI behavior",
    ordinal: 0,
    content_hash: "hash_101",
    created_at: 1_700_000_000,
    updated_at: 1_700_000_001,
  },
  current_semantics: null,
  provenance_actions: [],
  supporting_signals: [],
  validation_history: [],
  legacy_untracked: false,
  legacy_reason: null,
}

const ontologyFilters: OntologyRouteFilters = { includeAll: false, groupBy: "label" }

const health: HealthReport = {
  ok: true,
  db: "ok",
  version: "3.0.0",
  db_path: "/fixture/kanban.db",
  db_fingerprint: "sha256:storybook",
}

const maintenanceStatus: MaintenanceStatus = {
  database_instance_id: "db_storybook",
  protocol_version: 2,
  owner: { owner: null, mode: null, lease_expires_at: null, fence_epoch: 0, build_identity: null, last_heartbeat_at: null, active: false },
  stores: [],
}

const maintenanceStats: QueueStats = {
  board_id: boardIdentity.id,
  generated_at: 1_700_000_000_000,
  status_counts: [{ status: "running", count: 1 }],
  stale_claims: [],
  blocked_reasons: [],
  unplanned_active_tasks: 0,
  active_parents_with_incomplete_required_steps: 0,
}

const maintenanceSearch: SearchStatus = {
  backend: "projection-store",
  database_instance_id: "db_storybook",
  derived_index: true,
  stale: false,
  protocol_version: 2,
  generation: "generation-storybook",
  resolved_board_id: boardIdentity.id,
  fallback_reason: null,
  index_version: "v1",
  last_event_id: 2,
  index_lag_events: 0,
  message: "Projection is ready.",
}

const maintenanceDoctor: DoctorReport = {
  ok: true,
  integrity_check: "ok",
  migration_version: 1,
  user_version: 1,
  expired_running_tasks: 0,
  running_tasks_without_active_run: 0,
  orphan_running_runs: 0,
  dependency_cycles: 0,
  archived_dependency_edges: 0,
  missing_run_logs: 0,
  suspicious_run_log_paths: 0,
  executable_dependency_violations: 0,
  executable_spec_violations: 0,
  executable_schedule_violations: 0,
  unplanned_active_tasks: 0,
  active_parents_with_incomplete_required_steps: 0,
  outbox_pending: 0,
  outbox_running: 0,
  outbox_failed: 0,
  derived_dirty_stores: 0,
  derived_error_stores: 0,
  derived_stores: [],
  consistency_errors: 0,
  consistency_warnings: 0,
  consistency_issues: [],
  ontology_ledger_errors: 0,
  ontology_ledger_warnings: 0,
  ontology_ledger_issues: [],
}

const maintenanceApi: MaintenanceApi = {
  status: async () => maintenanceStatus,
  doctor: async () => maintenanceDoctor,
  stats: async () => maintenanceStats,
  searchStatus: async () => maintenanceSearch,
  checkpoint: async () => ({ busy: 0, log_frames: 2, checkpointed_frames: 2 }),
  backup: async () => ({ out_path: "/fixture/backup.sqlite", checksum_sha256: "sha256:backup", bytes: 1, source_fingerprint: "sha256:source" }),
  exportData: async () => ({ out_path: "/fixture/export.jsonl", checksum_sha256: "sha256:export", bytes: 1, record_count: 1, source_fingerprint: "sha256:source" }),
  importData: async () => ({ in_path: "/fixture/import.jsonl", source_fingerprint: "sha256:source", imported_records: 0, skipped_records: 0, rebuild_jobs_enqueued: 0, journal_id: "journal-storybook", phase: "staged", restart_required: false, staged_database_path: null, target_fingerprint_before: null, staged_fingerprint: null, publish_preconditions: [] }),
  vacuum: async () => ({ ok: true, source_fingerprint: "sha256:source", before_bytes: 2, after_bytes: 1 }),
  maintenanceRun: async () => ({ database_instance_id: "db_storybook", protocol_version: 2, owner: "storybook", mode: "run", action: "run", processed: 0, phase: "ready", degraded: false, errors: [], stores: [] }),
  maintenanceRebuild: async () => ({ database_instance_id: "db_storybook", protocol_version: 2, owner: "storybook", mode: "rebuild", action: "rebuild", processed: 0, phase: "ready", degraded: false, errors: [], stores: [] }),
  maintenanceCleanup: async () => ({ database_instance_id: "db_storybook", protocol_version: 2, owner: "storybook", mode: "cleanup", action: "cleanup", processed: 0, phase: "ready", degraded: false, errors: [], stores: [] }),
}

const noop = () => undefined

export const HomeReady: Story = {
  render: () => <PageStoryFrame><ProjectsCollection projects={projects} onRetry={noop} /></PageStoryFrame>,
}

export const HomeLoading: Story = {
  render: () => <PageStoryFrame><ProjectsCollection projects={[]} status="loading" /></PageStoryFrame>,
}

export const HomeEmpty: Story = {
  render: () => <PageStoryFrame><ProjectsCollection projects={[]} status="ready" /></PageStoryFrame>,
}

export const HomeError: Story = {
  render: () => <PageStoryFrame><ProjectsCollection projects={[]} status="error" onRetry={noop} /></PageStoryFrame>,
}

export const ProjectOverviewReady: Story = {
  render: () => <PageStoryFrame><ProjectOverview project={project} onOpenTasks={noop} /></PageStoryFrame>,
}

export const ProjectOverviewOffline: Story = {
  render: () => <PageStoryFrame><ProjectOverview project={project} status="offline" onRetry={noop} /></PageStoryFrame>,
}

export const BoardReady: Story = {
  render: () => <PageStoryFrame><BoardView state={boardReady} onSelectTask={noop} presentation="standalone" /></PageStoryFrame>,
}

export const BoardLoading: Story = {
  render: () => <PageStoryFrame><BoardView state={{ kind: "loading" }} /></PageStoryFrame>,
}

export const BoardEmpty: Story = {
  render: () => <PageStoryFrame><BoardView state={boardEmpty} /></PageStoryFrame>,
}

export const BoardError: Story = {
  render: () => <PageStoryFrame><BoardView state={{ kind: "error", message: "The board read model could not be loaded." }} onRetry={noop} /></PageStoryFrame>,
}

export const ListReady: Story = {
  render: () => <PageStoryFrame><TaskListView state={listState} rows={listRows} loading={false} displayVariant="grouped" onQueryChange={updateListQuery} onSelectTask={noop} /></PageStoryFrame>,
}

/** The table is a projection of the same list rows, not a second route or read. */
export const ListTable: Story = {
  render: () => <PageStoryFrame><TaskListView state={listState} rows={listRows} loading={false} displayVariant="table" onQueryChange={updateListQuery} onSelectTask={noop} /></PageStoryFrame>,
}

export const ListLoading: Story = {
  render: () => <PageStoryFrame><TaskListView state={listState} rows={[]} loading onQueryChange={updateListQuery} onSelectTask={noop} /></PageStoryFrame>,
}

export const ListEmpty: Story = {
  render: () => <PageStoryFrame><TaskListView state={listState} rows={[]} loading={false} onQueryChange={updateListQuery} onSelectTask={noop} /></PageStoryFrame>,
}

export const ListError: Story = {
  render: () => <PageStoryFrame><TaskListView state={listState} rows={[]} loading={false} error={new Error("Task list fixture error")} onRetry={noop} onQueryChange={updateListQuery} onSelectTask={noop} /></PageStoryFrame>,
}

export const MapReady: Story = {
  render: () => <PageStoryFrame><TaskMapPresentation board={boardIdentity.slug} taskId="t_201" state={mapReady} onSelectTask={noop} /></PageStoryFrame>,
}

export const MapLoading: Story = {
  render: () => <PageStoryFrame><TaskMapPresentation board={boardIdentity.slug} taskId={null} state={mapLoading} onSelectTask={noop} /></PageStoryFrame>,
}

export const MapError: Story = {
  render: () => <PageStoryFrame><TaskMapPresentation board={boardIdentity.slug} taskId={null} state={{ data: null, loading: false, error: new Error("Map fixture error") }} onSelectTask={noop} onRetry={noop} /></PageStoryFrame>,
}

export const RunsReady: Story = {
  render: () => <PageStoryFrame><TaskRunsPresentation locale="zh" taskId="t_101" state={runsReady} onRetry={noop} /></PageStoryFrame>,
}

export const RunsLoading: Story = {
  render: () => <PageStoryFrame><TaskRunsPresentation locale="zh" taskId="t_101" state={{ data: null, loading: true, error: null }} onRetry={noop} /></PageStoryFrame>,
}

export const RunsEmpty: Story = {
  render: () => <PageStoryFrame><TaskRunsPresentation locale="zh" taskId="t_101" state={{ data: { taskId: "t_101", runs: [], selectedRunId: null, log: null }, loading: false, error: null }} /></PageStoryFrame>,
}

export const RunsError: Story = {
  render: () => <PageStoryFrame><TaskRunsPresentation locale="zh" taskId="t_101" state={{ data: null, loading: false, error: new Error("Runs fixture error") }} onRetry={noop} /></PageStoryFrame>,
}

export const EventsReady: Story = {
  render: () => <PageStoryFrame><EventsPresentation locale="zh" taskId={null} kindFilter="" state={eventsReady} online onRefresh={noop} onSelectTask={noop} /></PageStoryFrame>,
}

export const EventsOffline: Story = {
  render: () => <PageStoryFrame><EventsPresentation locale="zh" taskId={null} kindFilter="" state={{ data: null, loading: false, error: null, stale: false }} online={false} onRefresh={noop} /></PageStoryFrame>,
}

export const EventsRecovering: Story = {
  render: () => <PageStoryFrame><EventsPresentation locale="zh" taskId={null} kindFilter="" state={{ ...eventsReady, loading: true, stale: true, error: new Error("Recovering event snapshot") }} online onRefresh={noop} /></PageStoryFrame>,
}

export const SignalsReady: Story = {
  render: () => <PageStoryFrame><SignalsScreenView boardName={boardIdentity.name} filters={signalFilters} list={readState([signal])} detail={readState(signal)} selectedSignalId={signal.id} online onRefresh={noop} onFiltersChange={noop} onSelectSignal={noop} onCloseDetail={noop} /></PageStoryFrame>,
}

export const SignalsEmpty: Story = {
  render: () => <PageStoryFrame><SignalsScreenView boardName={boardIdentity.name} filters={signalFilters} list={readState([])} detail={readState(null)} selectedSignalId={null} online onRefresh={noop} onFiltersChange={noop} onSelectSignal={noop} /></PageStoryFrame>,
}

export const SignalsError: Story = {
  render: () => <PageStoryFrame><SignalsScreenView boardName={boardIdentity.name} filters={signalFilters} list={readState([], "error", new Error("Signals fixture error"))} detail={readState(null)} selectedSignalId={null} online onRefresh={noop} onFiltersChange={noop} onSelectSignal={noop} /></PageStoryFrame>,
}

export const OntologyReady: Story = {
  render: () => <PageStoryFrame><OntologyStoryContent /></PageStoryFrame>,
}

function OntologyStoryContent() {
  const [selectedSignalId, setSelectedSignalId] = useState<string | null>(ontologySignal.id)
  const [atomRef, setAtomRef] = useState("hash_101")
  return (
    <OntologyScreenView
      boardName={boardIdentity.name}
      filters={ontologyFilters}
      signals={readState([ontologySignal])}
      groups={readState([ontologyGroup])}
      detail={readState(ontologyDetail)}
      atom={readState(atom)}
      selectedSignalId={selectedSignalId}
      atomRef={atomRef}
      online
      locale="zh"
      actionReason=""
      actionPending={false}
      onRefresh={noop}
      onFiltersChange={noop}
      onSelectSignal={setSelectedSignalId}
      onCloseDetail={() => setSelectedSignalId(null)}
      onActionReasonChange={noop}
      onLifecycleAction={noop}
      onExplainAtom={(nextAtomRef) => setAtomRef(nextAtomRef ?? "")}
      onAtomSearch={noop}
    />
  )
}

export const OntologyLoading: Story = {
  render: () => <PageStoryFrame><OntologyScreenView boardName={boardIdentity.name} filters={ontologyFilters} signals={readState([], "loading")} groups={readState([], "loading")} detail={readState(null, "loading")} atom={readState(null, "loading")} selectedSignalId={null} atomRef="" online locale="zh" actionReason="" actionPending={false} onRefresh={noop} onFiltersChange={noop} onSelectSignal={noop} onActionReasonChange={noop} onLifecycleAction={noop} onExplainAtom={noop} onAtomSearch={noop} /></PageStoryFrame>,
}

export const OntologyEmpty: Story = {
  render: () => <PageStoryFrame><OntologyScreenView boardName={boardIdentity.name} filters={ontologyFilters} signals={readState([])} groups={readState([])} detail={readState(null)} atom={readState(null)} selectedSignalId={null} atomRef="" online locale="zh" actionReason="" actionPending={false} onRefresh={noop} onFiltersChange={noop} onSelectSignal={noop} onActionReasonChange={noop} onLifecycleAction={noop} onExplainAtom={noop} onAtomSearch={noop} /></PageStoryFrame>,
}

export const HealthReady: Story = {
  render: () => <PageStoryFrame><HealthPage runtime={runtime} initialReport={health} /></PageStoryFrame>,
}

export const HealthLoading: Story = {
  render: () => <PageStoryFrame><HealthPage runtime={runtime} read={() => new Promise<HealthReport>(() => undefined)} /></PageStoryFrame>,
}

export const MaintenanceReady: Story = {
  render: () => <PageStoryFrame><MaintenancePage runtime={runtime} boardSlug={boardIdentity.slug} api={maintenanceApi} initial={{ status: maintenanceStatus, stats: maintenanceStats, searchStatus: maintenanceSearch, doctor: maintenanceDoctor }} /></PageStoryFrame>,
}

export const MaintenanceConfirm: Story = {
  render: () => <PageStoryFrame><MaintenancePage runtime={runtime} boardSlug={boardIdentity.slug} api={maintenanceApi} initial={{ status: maintenanceStatus, stats: maintenanceStats, searchStatus: maintenanceSearch, doctor: maintenanceDoctor }} /></PageStoryFrame>,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    await userEvent.click(await canvas.findByTestId("maintenance-run-submit"))
    await canvas.findByTestId("maintenance-confirm-dialog")
  },
}

export const SettingsReady: Story = {
  render: () => <PageStoryFrame><SettingsPage runtime={runtime} initialHealth={health} onNavigate={noop} onReconnect={() => "already-live"} clipboardWrite={noop} /></PageStoryFrame>,
}

export const SettingsError: Story = {
  render: () => <PageStoryFrame><SettingsPage runtime={runtime} read={async () => { throw new Error("Settings health fixture error") }} onNavigate={noop} clipboardWrite={noop} /></PageStoryFrame>,
}
