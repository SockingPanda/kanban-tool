import { FileText } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Card } from "@/components/ui/card"
import { PageToolbar, TaskIdentityLine } from "@/components/ui/composites"
import { Empty, EmptyDescription } from "@/components/ui/empty"
import { Item, ItemActions, ItemContent, ItemTitle } from "@/components/ui/item"
import { ScrollArea } from "@/components/ui/scroll-area"
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
    <div className="flex min-h-0 flex-1 flex-col bg-card">
      <PageToolbar
        title="Runs"
        description={
          selectedTask ? (
            <TaskIdentityLine id={selectedTask.id} ref={selectedTask.ref} seq={selectedTask.seq} />
          ) : "Select a task to inspect runs."
        }
      />
      <div className="grid min-h-0 flex-1 grid-cols-[360px_1fr]">
        <ScrollArea className="border-r border-border p-3">
          {detail.runs.length ? detail.runs.map((run) => <RunRow key={run.id} run={run} />) : (
            <Empty>
              <EmptyDescription>No runs for the selected task.</EmptyDescription>
            </Empty>
          )}
        </ScrollArea>
        <ScrollArea className="p-3">
          {detail.runLog ? (
            <div className="rounded-md border border-border bg-terminal-bg p-3 text-xs text-terminal-fg">
              <div className="mb-2 flex items-center justify-between text-terminal-muted-foreground">
                <span className="flex items-center gap-1">
                  <FileText className="h-3.5 w-3.5" />
                  {shortId(detail.runLog.run_id)}
                </span>
                {detail.runLog.truncated ? <span>truncated</span> : null}
              </div>
              <pre className="whitespace-pre-wrap font-mono leading-relaxed">{detail.runLog.content || "(empty)"}</pre>
            </div>
          ) : (
            <Empty>
              <EmptyDescription>No log available for the selected task.</EmptyDescription>
            </Empty>
          )}
        </ScrollArea>
      </div>
    </div>
  )
}

function RunRow({ run }: { run: Run }) {
  return (
    <Card className="mb-2 p-3 text-sm">
      <div className="mb-2 flex items-center justify-between">
        <span className="font-medium">{shortId(run.id)}</span>
        <Badge variant={runBadgeVariant(run.status)}>{run.status}</Badge>
      </div>
      <InfoRow label="worker" value={run.worker_profile ?? "manual"} />
      <InfoRow label="owner" value={run.claim_owner} />
      <InfoRow label="started" value={formatRelativeTime(run.started_at)} />
      <InfoRow label="finished" value={run.finished_at ? formatRelativeTime(run.finished_at) : "-"} />
      <InfoRow label="exit" value={run.exit_code === null ? "-" : String(run.exit_code)} />
      {run.error ? <div className="mt-2 text-xs text-destructive">{run.error}</div> : null}
    </Card>
  )
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <Item className="px-0 py-0 text-xs">
      <ItemContent>
        <ItemTitle className="text-xs font-normal text-muted-foreground">{label}</ItemTitle>
      </ItemContent>
      <ItemActions className="min-w-0">
        <span className="truncate font-medium">{value}</span>
      </ItemActions>
    </Item>
  )
}

function runBadgeVariant(status: string): "running" | "ready" | "secondary" {
  if (status === "running") return "running"
  if (status === "succeeded") return "ready"
  return "secondary"
}
