import { FileText } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import type { Run, Task } from "@/lib/api"
import { formatRelativeTime, shortId } from "@/lib/utils"

import type { DetailState } from "@/features/task-detail/detail-state"

export function RunsView({
  selectedTask,
  detail,
}: {
  selectedTask: Task | null
  detail: DetailState
}) {
  return (
    <div className="flex min-h-0 flex-1 flex-col bg-white">
      <div className="border-b border-neutral-200 px-4 py-3">
        <div className="text-sm font-medium">Runs</div>
        <div className="text-xs text-neutral-500">
          {selectedTask ? `Selected task #${selectedTask.seq} · ${shortId(selectedTask.id)}` : "Select a task to inspect runs."}
        </div>
      </div>
      <div className="grid min-h-0 flex-1 grid-cols-[360px_1fr]">
        <div className="min-h-0 overflow-auto border-r border-neutral-200 p-3">
          {detail.runs.length ? detail.runs.map((run) => <RunRow key={run.id} run={run} />) : (
            <div className="text-sm text-neutral-500">No runs for the selected task.</div>
          )}
        </div>
        <div className="min-h-0 overflow-auto p-3">
          {detail.runLog ? (
            <div className="rounded-md border border-neutral-200 bg-neutral-950 p-3 text-xs text-neutral-50">
              <div className="mb-2 flex items-center justify-between text-neutral-400">
                <span className="flex items-center gap-1">
                  <FileText className="h-3.5 w-3.5" />
                  {shortId(detail.runLog.run_id)}
                </span>
                {detail.runLog.truncated ? <span>truncated</span> : null}
              </div>
              <pre className="whitespace-pre-wrap font-mono leading-relaxed">{detail.runLog.content || "(empty)"}</pre>
            </div>
          ) : (
            <div className="text-sm text-neutral-500">No log available for the selected task.</div>
          )}
        </div>
      </div>
    </div>
  )
}

function RunRow({ run }: { run: Run }) {
  return (
    <div className="mb-2 rounded-md border border-neutral-200 bg-neutral-50 p-3 text-sm">
      <div className="mb-2 flex items-center justify-between">
        <span className="font-medium">{shortId(run.id)}</span>
        <Badge variant={runBadgeVariant(run.status)}>{run.status}</Badge>
      </div>
      <InfoRow label="worker" value={run.worker_profile ?? "manual"} />
      <InfoRow label="owner" value={run.claim_owner} />
      <InfoRow label="started" value={formatRelativeTime(run.started_at)} />
      <InfoRow label="finished" value={run.finished_at ? formatRelativeTime(run.finished_at) : "-"} />
      <InfoRow label="exit" value={run.exit_code === null ? "-" : String(run.exit_code)} />
      {run.error ? <div className="mt-2 text-xs text-red-700">{run.error}</div> : null}
    </div>
  )
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between gap-3 text-xs">
      <span className="text-neutral-500">{label}</span>
      <span className="truncate font-medium">{value}</span>
    </div>
  )
}

function runBadgeVariant(status: string): "running" | "ready" | "secondary" {
  if (status === "running") return "running"
  if (status === "succeeded") return "ready"
  return "secondary"
}
