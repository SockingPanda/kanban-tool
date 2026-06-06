import { FormEvent, useCallback, useEffect, useMemo, useState } from "react"
import {
  Activity,
  Archive,
  CheckCircle2,
  CircleDot,
  Command,
  Database,
  GitBranch,
  HeartPulse,
  Inbox,
  ListChecks,
  Loader2,
  PauseCircle,
  Play,
  Plus,
  RefreshCcw,
  Search,
  Settings,
  ShieldAlert,
  SquareKanban,
  TerminalSquare,
  XCircle,
} from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Separator } from "@/components/ui/separator"
import { Textarea } from "@/components/ui/textarea"
import {
  ApiError,
  ClaimResponse,
  Dependencies,
  EventRecord,
  KanbanApi,
  Run,
  RuntimeConfig,
  Task,
  TaskStatus,
  loadRuntimeConfig,
} from "@/lib/api"
import {
  blockTaskBody,
  canArchiveTask,
  canBlockTask,
  canCompleteTask,
  completeTaskBody,
} from "@/lib/action-policy"
import { cn, formatRelativeTime, shortId } from "@/lib/utils"

const columns: Array<{ id: TaskStatus; title: string; hint: string }> = [
  { id: "triage", title: "Triage", hint: "needs spec" },
  { id: "ready", title: "Ready", hint: "claimable" },
  { id: "running", title: "Running", hint: "active runs" },
  { id: "review", title: "Review", hint: "manual check" },
  { id: "blocked", title: "Blocked", hint: "needs input" },
  { id: "done", title: "Done", hint: "finished" },
]

const statusAccent: Record<TaskStatus, string> = {
  triage: "bg-neutral-400",
  todo: "bg-stone-500",
  scheduled: "bg-indigo-500",
  ready: "bg-emerald-500",
  running: "bg-sky-500",
  blocked: "bg-red-500",
  review: "bg-amber-500",
  done: "bg-lime-600",
  archived: "bg-neutral-300",
}

type DetailState = {
  dependencies: Dependencies
  runs: Run[]
  events: EventRecord[]
}

const emptyDetail: DetailState = {
  dependencies: { parents: [], children: [] },
  runs: [],
  events: [],
}

function App() {
  const [config, setConfig] = useState<RuntimeConfig | null>(null)
  const [api, setApi] = useState<KanbanApi | null>(null)
  const [tasks, setTasks] = useState<Task[]>([])
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [detail, setDetail] = useState<DetailState>(emptyDetail)
  const [search, setSearch] = useState("")
  const [newTitle, setNewTitle] = useState("")
  const [newDescription, setNewDescription] = useState("")
  const [blockReason, setBlockReason] = useState("")
  const [dependencyInput, setDependencyInput] = useState("")
  const [claimTokens, setClaimTokens] = useState<Record<string, string>>({})
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    loadRuntimeConfig()
      .then((runtime) => {
        const client = new KanbanApi(runtime)
        setConfig(runtime)
        setApi(client)
      })
      .catch((err: unknown) => setError(errorMessage(err)))
  }, [])

  const refreshTasks = useCallback(async () => {
    if (!api) return
    const nextTasks = await api.listTasks(search)
    setTasks(nextTasks)
    setSelectedId((current) => current ?? nextTasks[0]?.id ?? null)
  }, [api, search])

  const refreshDetail = useCallback(
    async (taskId: string) => {
      if (!api) return
      const [dependencies, runs, events] = await Promise.all([
        api.listDependencies(taskId),
        api.listRuns(taskId),
        api.listEvents(taskId),
      ])
      setDetail({ dependencies, runs, events })
    },
    [api],
  )

  useEffect(() => {
    if (!api) return
    setBusy(true)
    refreshTasks()
      .catch((err: unknown) => setError(errorMessage(err)))
      .finally(() => setBusy(false))
  }, [api, refreshTasks])

  useEffect(() => {
    if (!selectedId) {
      setDetail(emptyDetail)
      return
    }
    refreshDetail(selectedId).catch((err: unknown) => setError(errorMessage(err)))
  }, [refreshDetail, selectedId])

  const selectedTask = useMemo(
    () => tasks.find((task) => task.id === selectedId) ?? tasks[0] ?? null,
    [selectedId, tasks],
  )

  const grouped = useMemo(() => {
    const map = new Map<TaskStatus, Task[]>()
    for (const column of columns) map.set(column.id, [])
    for (const task of tasks) {
      const target = task.status === "todo" || task.status === "scheduled" ? "triage" : task.status
      if (map.has(target)) map.get(target)!.push(task)
    }
    return map
  }, [tasks])

  async function runAction(action: () => Promise<unknown>) {
    setBusy(true)
    setError(null)
    try {
      const result = await action()
      if (isClaimResponse(result)) {
        setClaimTokens((current) => ({ ...current, [result.task.id]: result.claim_token }))
      }
      await refreshTasks()
      if (selectedTask) await refreshDetail(selectedTask.id)
    } catch (err) {
      setError(errorMessage(err))
    } finally {
      setBusy(false)
    }
  }

  async function createTask(event: FormEvent) {
    event.preventDefault()
    if (!api || !newTitle.trim()) return
    await runAction(async () => {
      const task = await api.createTask({
        title: newTitle.trim(),
        description: newDescription.trim() || undefined,
      })
      setSelectedId(task.id)
      setNewTitle("")
      setNewDescription("")
    })
  }

  async function addDependency() {
    if (!api || !selectedTask || !dependencyInput.trim()) return
    await runAction(async () => {
      await api.addDependency(selectedTask.id, dependencyInput.trim())
      setDependencyInput("")
    })
  }

  const activeRun = detail.runs.find((run) => run.status === "running") ?? detail.runs[0]
  const queueCounts = {
    ready: tasks.filter((task) => task.status === "ready").length,
    running: tasks.filter((task) => task.status === "running").length,
    blocked: tasks.filter((task) => task.status === "blocked").length,
  }

  return (
    <div className="flex h-screen bg-[#f7f7f5] text-neutral-950">
      <aside className="flex w-52 shrink-0 flex-col border-r border-neutral-200 bg-[#fbfbfa]">
        <div className="flex h-14 items-center gap-2 px-4">
          <div className="flex h-7 w-7 items-center justify-center rounded-md bg-neutral-950 text-sm font-semibold text-white">
            kb
          </div>
          <div>
            <div className="text-sm font-semibold">Kanban Tool</div>
            <div className="text-xs text-neutral-500">local queue</div>
          </div>
        </div>
        <nav className="space-y-1 px-2 py-3">
          <NavItem icon={SquareKanban} label="Board" active />
          <NavItem icon={Inbox} label="Ready Queue" />
          <NavItem icon={ShieldAlert} label="Blocked" />
          <NavItem icon={TerminalSquare} label="Runs" />
          <NavItem icon={Activity} label="Events" />
          <NavItem icon={Settings} label="Settings" />
        </nav>
        <div className="mt-auto space-y-3 border-t border-neutral-200 p-3 text-xs text-neutral-500">
          <div className="flex items-center gap-2">
            <Database className="h-3.5 w-3.5" />
            <span className="truncate">{config?.dbPath ?? "loading db"}</span>
          </div>
          <div className="flex items-center justify-between">
            <span className="flex items-center gap-2">
              <span className="h-2 w-2 rounded-full bg-emerald-500" />
              API
            </span>
            <span>{config ? apiEndpointLabel(config.apiBaseUrl) : "-"}</span>
          </div>
        </div>
      </aside>

      <main className="flex min-w-0 flex-1 flex-col">
        <header className="flex h-14 items-center gap-3 border-b border-neutral-200 bg-white px-4">
          <div className="relative w-80">
            <Search className="pointer-events-none absolute left-2.5 top-2 h-4 w-4 text-neutral-400" />
            <Input
              className="pl-8"
              placeholder="Search tasks"
              value={search}
              onChange={(event) => setSearch(event.target.value)}
            />
          </div>
          <Button variant="secondary" size="icon" onClick={() => void refreshTasks()}>
            {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <RefreshCcw className="h-4 w-4" />}
          </Button>
          <div className="flex rounded-md border border-neutral-200 bg-neutral-50 p-0.5 text-sm">
            {["Board", "List", "Events"].map((view, index) => (
              <button
                key={view}
                className={cn(
                  "rounded px-3 py-1 text-neutral-500",
                  index === 0 && "bg-white text-neutral-950 shadow-sm",
                )}
              >
                {view}
              </button>
            ))}
          </div>
          <div className="ml-auto flex items-center gap-2">
            <Badge variant="secondary">actor {config?.actor ?? "-"}</Badge>
            <Badge variant="ready">dispatcher observed</Badge>
            <Button variant="ghost" size="icon">
              <Command className="h-4 w-4" />
            </Button>
          </div>
        </header>

        {error ? (
          <div className="border-b border-red-200 bg-red-50 px-4 py-2 text-sm text-red-700">{error}</div>
        ) : null}

        <div className="flex min-h-0 flex-1">
          <section className="flex min-w-0 flex-1 flex-col">
            <form onSubmit={createTask} className="grid grid-cols-[1fr_1.4fr_auto] gap-2 border-b border-neutral-200 bg-white px-4 py-3">
              <Input value={newTitle} onChange={(event) => setNewTitle(event.target.value)} placeholder="New task title" />
              <Input
                value={newDescription}
                onChange={(event) => setNewDescription(event.target.value)}
                placeholder="Optional spec or description"
              />
              <Button type="submit" disabled={!newTitle.trim() || busy}>
                <Plus className="h-4 w-4" />
                New task
              </Button>
            </form>

            <div className="grid min-h-0 flex-1 grid-cols-6 gap-px overflow-hidden bg-neutral-200">
              {columns.map((column) => (
                <BoardColumn
                  key={column.id}
                  column={column}
                  tasks={grouped.get(column.id) ?? []}
                  selectedId={selectedTask?.id}
                  dependencies={detail.dependencies}
                  onSelect={setSelectedId}
                />
              ))}
            </div>
          </section>

          <TaskDetail
            api={api}
            task={selectedTask}
            detail={detail}
            activeRun={activeRun}
            blockReason={blockReason}
            setBlockReason={setBlockReason}
            dependencyInput={dependencyInput}
            setDependencyInput={setDependencyInput}
            claimToken={selectedTask ? claimTokens[selectedTask.id] ?? null : null}
            busy={busy}
            onAction={runAction}
            onAddDependency={addDependency}
          />
        </div>

        <footer className="flex h-8 items-center justify-between border-t border-neutral-200 bg-white px-4 text-xs text-neutral-500">
          <span>Last refresh {new Date().toLocaleTimeString()}</span>
          <span>
            ready {queueCounts.ready} / running {queueCounts.running} / blocked {queueCounts.blocked}
          </span>
        </footer>
      </main>
    </div>
  )
}

function BoardColumn({
  column,
  tasks,
  selectedId,
  dependencies,
  onSelect,
}: {
  column: { id: TaskStatus; title: string; hint: string }
  tasks: Task[]
  selectedId?: string
  dependencies: Dependencies
  onSelect: (taskId: string) => void
}) {
  return (
    <div className="flex min-w-0 flex-col bg-[#f7f7f5]">
      <div className="border-b border-neutral-200 bg-white px-3 py-2">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <span className={cn("h-2 w-2 rounded-full", statusAccent[column.id])} />
            <span className="text-sm font-semibold">{column.title}</span>
          </div>
          <span className="text-xs text-neutral-500">{tasks.length}</span>
        </div>
        <div className="mt-0.5 text-xs text-neutral-500">{column.hint}</div>
      </div>
      <div className="min-h-0 flex-1 space-y-2 overflow-y-auto p-2">
        {tasks.map((task) => (
          <TaskCard
            key={task.id}
            task={task}
            selected={task.id === selectedId}
            dependencyCount={
              task.id === selectedId
                ? dependencies.parents.length + dependencies.children.length
                : undefined
            }
            onSelect={() => onSelect(task.id)}
          />
        ))}
      </div>
    </div>
  )
}

function TaskCard({
  task,
  selected,
  dependencyCount,
  onSelect,
}: {
  task: Task
  selected: boolean
  dependencyCount?: number
  onSelect: () => void
}) {
  return (
    <button
      className={cn(
        "w-full rounded-md border bg-white p-2 text-left transition-colors hover:border-neutral-300",
        selected ? "border-neutral-900 shadow-sm" : "border-neutral-200",
      )}
      onClick={onSelect}
    >
      <div className="flex items-start gap-2">
        <span className={cn("mt-1.5 h-2 w-2 rounded-full", statusAccent[task.status])} />
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-medium">#{task.seq} {task.title}</div>
          <div className="mt-1 flex flex-wrap gap-1 text-xs text-neutral-500">
            <span>P{task.priority}</span>
            {task.due_at ? <span>due {formatRelativeTime(task.due_at)}</span> : null}
            {task.scheduled_at ? <span>scheduled {formatRelativeTime(task.scheduled_at)}</span> : null}
            {task.status === "running" ? <span>heartbeat {formatRelativeTime(task.last_heartbeat_at)}</span> : null}
            {typeof dependencyCount === "number" ? <span>{dependencyCount} deps</span> : null}
          </div>
          {task.status_reason ? <div className="mt-1 line-clamp-2 text-xs text-red-700">{task.status_reason}</div> : null}
        </div>
      </div>
    </button>
  )
}

function TaskDetail({
  api,
  task,
  detail,
  activeRun,
  blockReason,
  setBlockReason,
  dependencyInput,
  setDependencyInput,
  claimToken,
  busy,
  onAction,
  onAddDependency,
}: {
  api: KanbanApi | null
  task: Task | null
  detail: DetailState
  activeRun?: Run
  blockReason: string
  setBlockReason: (value: string) => void
  dependencyInput: string
  setDependencyInput: (value: string) => void
  claimToken: string | null
  busy: boolean
  onAction: (action: () => Promise<unknown>) => Promise<void>
  onAddDependency: () => Promise<void>
}) {
  if (!task) {
    return <aside className="w-[420px] border-l border-neutral-200 bg-white p-4 text-sm text-neutral-500">No task selected.</aside>
  }

  const actions = legalActions(task, claimToken, blockReason)

  return (
    <aside className="flex w-[420px] shrink-0 flex-col border-l border-neutral-200 bg-white">
      <div className="border-b border-neutral-200 p-4">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="text-xs text-neutral-500">#{task.seq} {shortId(task.id)}</div>
            <h2 className="mt-1 text-lg font-semibold leading-snug">{task.title}</h2>
          </div>
          <Badge variant={badgeVariant(task.status)}>{task.status}</Badge>
        </div>
        <p className="mt-2 text-sm text-neutral-600">{task.description || "No description yet."}</p>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto p-4">
        <Section title="Legal transitions">
          <div className="grid grid-cols-2 gap-2">
            {actions.map((action) => (
              <Button
                key={action.label}
                variant={action.danger ? "destructive" : "secondary"}
                disabled={!api || busy || !action.enabled}
                onClick={() => {
                  if (!api) return
                  void onAction(() => action.run(api, task))
                }}
              >
                <action.icon className="h-4 w-4" />
                {action.label}
              </Button>
            ))}
          </div>
          {task.status === "blocked" ? (
            <div className="mt-2 text-xs text-neutral-500">Unblock asks the service to recompute schedule and dependency state.</div>
          ) : null}
          {task.status === "running" ? (
            <Textarea
              className="mt-3"
              placeholder="Block reason"
              value={blockReason}
              onChange={(event) => setBlockReason(event.target.value)}
            />
          ) : null}
        </Section>

        <Separator className="my-4" />

        <Section title="Dependencies">
          <div className="space-y-3">
            <DependencyGroup title="Parents" tasks={detail.dependencies.parents} />
            <DependencyGroup title="Children" tasks={detail.dependencies.children} />
            <div className="flex gap-2">
              <Input
                value={dependencyInput}
                onChange={(event) => setDependencyInput(event.target.value)}
                placeholder="Parent task id"
              />
              <Button variant="outline" disabled={!dependencyInput.trim() || busy} onClick={() => void onAddDependency()}>
                <GitBranch className="h-4 w-4" />
              </Button>
            </div>
          </div>
        </Section>

        <Separator className="my-4" />

        <Section title="Run summary">
          {activeRun ? (
            <div className="space-y-2 text-sm">
              <InfoRow label="run" value={shortId(activeRun.id)} />
              <InfoRow label="status" value={activeRun.status} />
              <InfoRow label="worker" value={activeRun.worker_profile ?? "manual"} />
              <InfoRow label="owner" value={activeRun.claim_owner} />
              <InfoRow label="started" value={formatRelativeTime(activeRun.started_at)} />
              <InfoRow label="log" value={activeRun.log_path ?? "-"} />
            </div>
          ) : (
            <div className="text-sm text-neutral-500">No runs yet.</div>
          )}
        </Section>

        <Separator className="my-4" />

        <Section title="Event timeline">
          <div className="space-y-2">
            {detail.events.slice().reverse().map((event) => (
              <div key={event.id} className="grid grid-cols-[auto_1fr] gap-2 text-sm">
                <CircleDot className="mt-0.5 h-4 w-4 text-neutral-400" />
                <div>
                  <div className="font-medium">{event.kind}</div>
                  <div className="text-xs text-neutral-500">
                    {formatRelativeTime(event.created_at)} by {event.actor ?? "system"}
                  </div>
                </div>
              </div>
            ))}
          </div>
        </Section>
      </div>
    </aside>
  )
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section>
      <h3 className="mb-2 text-xs font-semibold uppercase tracking-normal text-neutral-500">{title}</h3>
      {children}
    </section>
  )
}

function DependencyGroup({ title, tasks }: { title: string; tasks: Task[] }) {
  return (
    <div>
      <div className="mb-1 text-xs text-neutral-500">{title}</div>
      <div className="flex flex-wrap gap-1">
        {tasks.length ? (
          tasks.map((task) => (
            <Badge key={task.id} variant={task.status === "done" ? "ready" : task.status === "blocked" ? "blocked" : "secondary"}>
              #{task.seq} {task.status}
            </Badge>
          ))
        ) : (
          <span className="text-sm text-neutral-400">none</span>
        )}
      </div>
    </div>
  )
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between gap-3">
      <span className="text-neutral-500">{label}</span>
      <span className="truncate font-medium">{value}</span>
    </div>
  )
}

function NavItem({ icon: Icon, label, active = false }: { icon: React.ElementType; label: string; active?: boolean }) {
  return (
    <button
      className={cn(
        "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-neutral-600",
        active && "bg-neutral-100 text-neutral-950",
      )}
    >
      <Icon className="h-4 w-4" />
      {label}
    </button>
  )
}

function legalActions(task: Task, claimToken: string | null, blockReason: string) {
  return [
    {
      label: "Specify",
      icon: ListChecks,
      enabled: task.status === "triage",
      run: (api: KanbanApi, item: Task) => api.transition(item, "specify", { description: item.description ?? "ready spec" }),
    },
    {
      label: "Promote",
      icon: Play,
      enabled: task.status === "todo" || task.status === "scheduled",
      run: (api: KanbanApi, item: Task) => api.transition(item, "promote"),
    },
    {
      label: "Claim",
      icon: Play,
      enabled: task.status === "ready",
      run: (api: KanbanApi, item: Task) => api.transition(item, "claim", { ttl_ms: 300_000, worker_profile: "manual" }),
    },
    {
      label: "Heartbeat",
      icon: HeartPulse,
      enabled: task.status === "running" && Boolean(claimToken),
      run: (api: KanbanApi, item: Task) => api.transition(item, "heartbeat", { claim_token: claimToken, ttl_ms: 300_000 }),
    },
    {
      label: "Complete",
      icon: CheckCircle2,
      enabled: canCompleteTask(task.status, claimToken),
      run: (api: KanbanApi, item: Task) =>
        api.transition(item, "complete", completeTaskBody(item.status, claimToken)),
    },
    {
      label: "Review",
      icon: PauseCircle,
      enabled: task.status === "running" && Boolean(claimToken),
      run: (api: KanbanApi, item: Task) => api.transition(item, "submit-review", { claim_token: claimToken }),
    },
    {
      label: "Block",
      icon: XCircle,
      enabled: canBlockTask(task.status, claimToken, blockReason),
      danger: true,
      run: (api: KanbanApi, item: Task) =>
        api.transition(item, "block", blockTaskBody(claimToken, blockReason)),
    },
    {
      label: "Unblock",
      icon: RefreshCcw,
      enabled: task.status === "blocked",
      run: (api: KanbanApi, item: Task) => api.transition(item, "unblock"),
    },
    {
      label: "Archive",
      icon: Archive,
      enabled: canArchiveTask(task.status),
      danger: true,
      run: (api: KanbanApi, item: Task) => api.transition(item, "archive"),
    },
  ]
}

function badgeVariant(status: TaskStatus) {
  if (status === "ready" || status === "done") return "ready"
  if (status === "running") return "running"
  if (status === "blocked") return "blocked"
  if (status === "review") return "review"
  return "secondary"
}

function apiEndpointLabel(apiBaseUrl: string) {
  if (!apiBaseUrl) return "same-origin"
  return new URL(apiBaseUrl).port
}

function isClaimResponse(value: unknown): value is ClaimResponse {
  return Boolean(value && typeof value === "object" && "claim_token" in value)
}

function errorMessage(err: unknown) {
  if (err instanceof ApiError) return `${err.code}: ${err.message}`
  if (err instanceof Error) return err.message
  return String(err)
}

export default App
