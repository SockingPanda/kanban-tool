import { useState } from "react"
import { useQuery } from "@tanstack/react-query"
import { Download, Loader2, Trash2, Upload } from "lucide-react"

import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Empty, EmptyDescription } from "@/components/ui/empty"
import { Input } from "@/components/ui/input"
import { Skeleton } from "@/components/ui/skeleton"
import { useI18n } from "@/i18n"
import type { Attachment, DownloadedAttachment, KanbanApi, Task } from "@/lib/api"
import { presentApiError } from "@/lib/api/error-presentation"
import { queryKeys } from "@/lib/query-keys"
import { formatRelativeTime } from "@/lib/utils"

type AttachmentActionOptions = {
  label?: string
  fallbackTaskId?: string | null
  invalidate?: "none" | "attachments"
}

export function TaskAttachmentsPanel(props: {
  api: KanbanApi | null
  task: Task
  pending: boolean
  onAction: (action: () => Promise<unknown>, options?: AttachmentActionOptions) => Promise<unknown>
}) {
  if (!props.api) return <TaskAttachmentsUnavailable />
  return <ConnectedTaskAttachmentsPanel {...props} api={props.api} />
}

function TaskAttachmentsUnavailable() {
  const { t } = useI18n()
  return (
    <Alert>
      <AlertDescription>{t("Attachments are unavailable until the service is connected.")}</AlertDescription>
    </Alert>
  )
}

function ConnectedTaskAttachmentsPanel({
  api,
  task,
  pending,
  onAction,
}: {
  api: KanbanApi
  task: Task
  pending: boolean
  onAction: (action: () => Promise<unknown>, options?: AttachmentActionOptions) => Promise<unknown>
}) {
  const { t } = useI18n()
  const [selectedFile, setSelectedFile] = useState<File | null>(null)
  const [fileInputKey, setFileInputKey] = useState(0)
  const attachmentsQuery = useQuery({
    queryKey: queryKeys.taskAttachments(task.id),
    queryFn: ({ signal }) => api.listAttachments(task.id, { signal }),
  })

  async function uploadSelectedFile() {
    if (!selectedFile) return
    const content = Array.from(new Uint8Array(await selectedFile.arrayBuffer()))
    const result = await onAction(
      () =>
        api.createAttachment(task.id, {
          filename: selectedFile.name,
          content,
          content_type: selectedFile.type || null,
        }),
      { fallbackTaskId: task.id, label: "attachment", invalidate: "attachments" },
    )
    if (result) {
      setSelectedFile(null)
      setFileInputKey((current) => current + 1)
    }
  }

  async function downloadAttachment(attachment: Attachment) {
    const result = await onAction(
      () => api.downloadAttachment(task.id, attachment.id),
      { fallbackTaskId: task.id, label: "attachment", invalidate: "none" },
    )
    if (!isDownloadedAttachment(result)) return

    const blob = new Blob([result.content.buffer as ArrayBuffer], {
      type: result.content_type ?? attachment.content_type ?? "application/octet-stream",
    })
    const url = URL.createObjectURL(blob)
    const anchor = document.createElement("a")
    anchor.href = url
    anchor.download = attachment.filename
    anchor.click()
    anchor.remove()
    window.setTimeout(() => URL.revokeObjectURL(url), 0)
  }

  async function removeAttachment(attachment: Attachment) {
    await onAction(
      () => api.deleteAttachment(task.id, attachment.id),
      { fallbackTaskId: task.id, label: "attachment", invalidate: "attachments" },
    )
  }

  const attachments = attachmentsQuery.data ?? []
  return (
    <div className="min-w-0 space-y-3">
      <div className="flex min-w-0 flex-col gap-2 rounded-md border border-border bg-muted/20 p-2">
        <Input
          key={fileInputKey}
          type="file"
          aria-label={t("Choose file")}
          disabled={pending}
          onChange={(event) => setSelectedFile(event.currentTarget.files?.[0] ?? null)}
        />
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="self-start"
          disabled={!selectedFile || pending}
          onClick={() => void uploadSelectedFile()}
        >
          {pending ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Upload className="h-3.5 w-3.5" />}
          {pending ? t("Uploading…") : t("Upload attachment")}
        </Button>
      </div>

      {attachmentsQuery.error ? (
        <Alert className="border-destructive/50 bg-destructive/5">
          <AlertDescription className="break-words text-destructive">{presentApiError(attachmentsQuery.error, t)}</AlertDescription>
        </Alert>
      ) : null}

      {attachmentsQuery.isLoading ? (
        <div className="space-y-2">
          <Skeleton className="h-12 w-full" />
          <Skeleton className="h-12 w-full" />
        </div>
      ) : attachments.length ? (
        <div className="min-w-0 space-y-2">
          {attachments.map((attachment) => (
            <AttachmentRow
              key={attachment.id}
              attachment={attachment}
              pending={pending}
              onDownload={() => void downloadAttachment(attachment)}
              onRemove={() => void removeAttachment(attachment)}
            />
          ))}
        </div>
      ) : (
        <Empty className="items-start rounded-md border border-border bg-muted/20 p-3 text-left">
          <EmptyDescription>{t("No attachments yet.")}</EmptyDescription>
        </Empty>
      )}
    </div>
  )
}

function AttachmentRow({
  attachment,
  pending,
  onDownload,
  onRemove,
}: {
  attachment: Attachment
  pending: boolean
  onDownload: () => void
  onRemove: () => void
}) {
  const { t } = useI18n()
  return (
    <div className="min-w-0 rounded-md border border-border bg-card p-2">
      <div className="flex min-w-0 items-start justify-between gap-2">
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-medium" title={attachment.filename}>{attachment.filename}</div>
          <div className="mt-1 flex min-w-0 flex-wrap gap-x-2 gap-y-0.5 text-xs text-muted-foreground">
            <span>{formatAttachmentSize(attachment.size_bytes)}</span>
            {attachment.content_type ? <span className="max-w-full truncate">{attachment.content_type}</span> : null}
            <span>{formatRelativeTime(attachment.created_at)}</span>
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          <Button
            type="button"
            variant="outline"
            size="icon"
            aria-label={t("Download attachment")}
            title={t("Download attachment")}
            disabled={pending}
            onClick={onDownload}
          >
            <Download className="h-3.5 w-3.5" />
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            aria-label={t("Remove attachment")}
            title={t("Remove attachment")}
            disabled={pending}
            onClick={onRemove}
          >
            <Trash2 className="h-3.5 w-3.5 text-destructive" />
          </Button>
        </div>
      </div>
    </div>
  )
}

function formatAttachmentSize(size: number) {
  if (size < 1024) return `${size} B`
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`
  if (size < 1024 * 1024 * 1024) return `${(size / (1024 * 1024)).toFixed(1)} MB`
  return `${(size / (1024 * 1024 * 1024)).toFixed(1)} GB`
}

function isDownloadedAttachment(value: unknown): value is DownloadedAttachment {
  return Boolean(value && typeof value === "object" && "content" in value && value.content instanceof Uint8Array)
}
