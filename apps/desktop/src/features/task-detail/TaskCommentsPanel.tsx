import { memo, useMemo } from "react"
import { ChevronDown, MessageSquare } from "lucide-react"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { Empty, EmptyDescription } from "@/components/ui/empty"
import { Field, FieldLabel } from "@/components/ui/field"
import { InputGroup, InputGroupButton, InputGroupTextarea } from "@/components/ui/input-group"
import { MenuSelect, type MenuSelectOption } from "@/components/ui/menu-select"
import { useI18n } from "@/i18n"
import type { CommentRecord } from "@/lib/api"
import { cn, formatRelativeTime } from "@/lib/utils"

import { commentPageState, type CommentSortOrder } from "./comment-list-state"
import { MarkdownDescription } from "./markdown"

export function TaskCommentsPanel({
  commentsPage,
  commentSortOrder,
  setCommentSortOrder,
  setCommentPage,
  commentBody,
  setCommentBody,
  pendingAction,
  onAddComment,
}: {
  commentsPage: ReturnType<typeof commentPageState>
  commentSortOrder: CommentSortOrder
  setCommentSortOrder: (value: CommentSortOrder) => void
  setCommentPage: (value: number | ((current: number) => number)) => void
  commentBody: string
  setCommentBody: (value: string) => void
  pendingAction: string | null
  onAddComment: () => Promise<void>
}) {
  const { t } = useI18n()
  const commentSortOptions = useMemo<MenuSelectOption<CommentSortOrder>[]>(
    () => [
      { value: "newest", label: t("Newest first") },
      { value: "oldest", label: t("Oldest first") },
    ],
    [t],
  )

  return (
    <div className="space-y-3">
      {commentsPage.total ? (
        <div className="flex items-center justify-between gap-2">
          <div className="text-xs text-muted-foreground">
            {t(commentsPage.total === 1 ? "{count} comment" : "{count} comments", { count: commentsPage.total })}
          </div>
          <MenuSelect
            ariaLabel={t("Comment sort order")}
            options={commentSortOptions}
            value={commentSortOrder}
            onValueChange={(value) => {
              setCommentSortOrder(value)
              setCommentPage(0)
            }}
            triggerClassName="h-8 w-36"
          />
        </div>
      ) : null}
      <CommentRows commentsPage={commentsPage} />
      {commentsPage.pageCount > 1 ? <CommentPager commentsPage={commentsPage} setCommentPage={setCommentPage} /> : null}
      <Field>
        <FieldLabel>{t("Comment body")}</FieldLabel>
        <InputGroup>
          <InputGroupTextarea
            className="min-h-20 resize-y py-2"
            aria-label={t("Comment body")}
            name="comment-body"
            autoComplete="off"
            value={commentBody}
            onChange={(event) => setCommentBody(event.target.value)}
            placeholder={t("Add handoff note")}
          />
          <InputGroupButton
            className="h-auto self-stretch"
            variant="outline"
            aria-label={t("Add comment")}
            disabled={!commentBody.trim() || pendingAction === "comment"}
            onClick={() => void onAddComment()}
          >
            <MessageSquare className="h-4 w-4" />
          </InputGroupButton>
        </InputGroup>
      </Field>
    </div>
  )
}

function CommentRows({ commentsPage }: { commentsPage: ReturnType<typeof commentPageState> }) {
  const { t } = useI18n()
  return (
    <div className="space-y-2">
      {commentsPage.total ? (
        commentsPage.comments.map((comment) => (
          <Card key={comment.id} className="p-2 text-sm">
            <div className="mb-1 flex items-center justify-between text-xs text-muted-foreground">
              <span className="flex items-center gap-1.5">
                {comment.author}
                {comment.kind === "decision" ? <Badge variant="secondary">{t("decision")}</Badge> : null}
                {comment.kind === "signal" ? <Badge variant="secondary">{t("signal")}</Badge> : null}
              </span>
              <span>{formatRelativeTime(comment.created_at)}</span>
            </div>
            <MarkdownDescription className="mt-1 text-card-foreground">{comment.body}</MarkdownDescription>
            {comment.kind === "decision" ? <DecisionComment comment={comment} /> : null}
            {comment.kind === "signal" ? <SignalLinkComment comment={comment} /> : null}
          </Card>
        ))
      ) : (
        <Empty className="items-start p-0 text-left">
          <EmptyDescription>{t("No comments yet.")}</EmptyDescription>
        </Empty>
      )}
    </div>
  )
}

function CommentPager({
  commentsPage,
  setCommentPage,
}: {
  commentsPage: ReturnType<typeof commentPageState>
  setCommentPage: (value: number | ((current: number) => number)) => void
}) {
  const { t } = useI18n()
  return (
    <div className="flex items-center justify-between gap-2 text-xs text-muted-foreground">
      <Button variant="outline" size="sm" aria-label={t("Previous comments")} disabled={!commentsPage.hasPreviousPage} onClick={() => setCommentPage((current) => Math.max(0, current - 1))}>
        {t("Previous")}
      </Button>
      <span>
        {t("Page {current} of {total}", { current: commentsPage.page + 1, total: commentsPage.pageCount })}
      </span>
      <Button variant="outline" size="sm" aria-label={t("Next comments")} disabled={!commentsPage.hasNextPage} onClick={() => setCommentPage((current) => current + 1)}>
        {t("Next")}
      </Button>
    </div>
  )
}

type DecisionOption = {
  slug: string
  title: string
  detail: string
}

type DecisionMetadata = {
  options: DecisionOption[]
  selected: string
  reason: string
  risk?: string
  verification?: string
}

type ParsedDecision = { ok: true; metadata: DecisionMetadata } | { ok: false; error: string }

export const DecisionComment = memo(function DecisionComment({ comment }: { comment: CommentRecord }) {
  const { t } = useI18n()
  const decision = useMemo(() => parseDecisionMetadata(comment.metadata), [comment.metadata])
  if (!decision.ok) {
    return (
      <Alert className="mt-2 border-destructive/50 bg-destructive/5">
        <AlertTitle className="text-destructive">{t("Invalid decision metadata")}</AlertTitle>
        <AlertDescription className="text-destructive">{t(decision.error)}</AlertDescription>
      </Alert>
    )
  }

  const { metadata } = decision
  return (
    <div className="mt-2 space-y-2 rounded-md border border-border bg-muted/30 p-2">
      <div className="flex flex-wrap gap-1.5">
        {metadata.options.map((option) => {
          const selected = option.slug === metadata.selected
          return (
            <Collapsible key={option.slug} defaultOpen={selected}>
              <CollapsibleTrigger asChild>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className={cn(
                    "text-muted-foreground hover:bg-background",
                    selected && "border-[var(--status-ready-ring)] bg-[var(--status-ready-bg)] text-[var(--status-ready-fg)]",
                  )}
                  aria-label={t("Show decision option {slug}", { slug: option.slug })}
                >
                  {option.slug}
                  <ChevronDown className="h-3 w-3" />
                </Button>
              </CollapsibleTrigger>
              <CollapsibleContent
                className={cn(
                  "mt-1 max-w-full rounded-md border border-border bg-background p-2 text-xs",
                  selected && "border-[var(--status-ready-ring)] bg-[var(--status-ready-bg)] text-[var(--status-ready-fg)]",
                )}
              >
                <div className="font-medium text-foreground">{option.title}</div>
                <MarkdownDescription className="mt-1 text-xs text-muted-foreground">{option.detail}</MarkdownDescription>
              </CollapsibleContent>
            </Collapsible>
          )
        })}
      </div>
      <DecisionField label={t("reason")} value={metadata.reason} />
      {metadata.risk ? <DecisionField label={t("risk")} value={metadata.risk} /> : null}
      {metadata.verification ? <DecisionField label={t("verification")} value={metadata.verification} /> : null}
    </div>
  )
})

function DecisionField({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid grid-cols-[6rem_1fr] gap-2 text-xs">
      <div className="font-medium uppercase tracking-normal text-muted-foreground">{label}</div>
      <MarkdownDescription className="mt-0 text-xs">{value}</MarkdownDescription>
    </div>
  )
}

function parseDecisionMetadata(parsed: Record<string, unknown>): ParsedDecision {
  const options = parsed.options
  if (!Array.isArray(options) || options.length === 0) {
    return { ok: false, error: "options must be a non-empty array" }
  }

  const seen = new Set<string>()
  const decisionOptions: DecisionOption[] = []
  for (const option of options) {
    if (!isObject(option)) return { ok: false, error: "options must be objects" }
    const slug = nonEmptyRawString(option.slug)
    const title = nonEmptyString(option.title)
    const detail = nonEmptyString(option.detail)
    if (!slug || !title || !detail) return { ok: false, error: "each option needs slug, title, and detail" }
    if (!isDecisionSlug(slug)) return { ok: false, error: "option slug must be lowercase ASCII letters, digits, or hyphen" }
    if (seen.has(slug)) return { ok: false, error: "option slugs must be unique" }
    seen.add(slug)
    decisionOptions.push({ slug, title, detail })
  }

  const selected = nonEmptyRawString(parsed.selected)
  if (!selected || !seen.has(selected)) return { ok: false, error: "selected must match an option slug" }
  const reason = nonEmptyString(parsed.reason)
  if (!reason) return { ok: false, error: "reason must be a non-empty string" }
  const risk = optionalNonEmptyString(parsed.risk)
  if (risk === false) return { ok: false, error: "risk must be a non-empty string" }
  const verification = optionalNonEmptyString(parsed.verification)
  if (verification === false) return { ok: false, error: "verification must be a non-empty string" }

  return { ok: true, metadata: { options: decisionOptions, selected, reason, risk, verification } }
}

type SignalLinkMetadata = {
  type: "signal_link"
  signal_id: string
  observation_id: string
  signal_kind: string
  signal_status: "open" | "confirmed" | "rejected" | "superseded" | "resolved"
}

function SignalLinkComment({ comment }: { comment: CommentRecord }) {
  const { t } = useI18n()
  const metadata = parseSignalLinkMetadata(comment.metadata)
  if (!metadata) return null
  return (
    <div className="mt-2 grid gap-1 rounded-md border border-border bg-muted/30 p-2 text-xs">
      <DecisionField label={t("signal")} value={metadata.signal_id} />
      <DecisionField label={t("kind")} value={metadata.signal_kind} />
      <DecisionField label={t("status")} value={t(metadata.signal_status)} />
    </div>
  )
}

function parseSignalLinkMetadata(metadata: Record<string, unknown>): SignalLinkMetadata | null {
  const expectedKeys = ["type", "signal_id", "observation_id", "signal_kind", "signal_status"]
  const keys = Object.keys(metadata)
  if (keys.length !== expectedKeys.length || keys.some((key) => !expectedKeys.includes(key))) return null
  if (metadata.type !== "signal_link") return null
  if (typeof metadata.signal_id !== "string" || !metadata.signal_id) return null
  if (typeof metadata.observation_id !== "string" || !metadata.observation_id) return null
  if (typeof metadata.signal_kind !== "string" || !metadata.signal_kind) return null
  if (!isSignalLinkStatus(metadata.signal_status)) return null
  return metadata as SignalLinkMetadata
}

function isSignalLinkStatus(value: unknown): value is SignalLinkMetadata["signal_status"] {
  return value === "open" || value === "confirmed" || value === "rejected" || value === "superseded" || value === "resolved"
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function nonEmptyString(value: unknown) {
  return typeof value === "string" && value.trim() ? value.trim() : null
}

function nonEmptyRawString(value: unknown) {
  return typeof value === "string" && value.trim() ? value : null
}

function optionalNonEmptyString(value: unknown) {
  if (value === undefined) return undefined
  return nonEmptyString(value) ?? false
}

function isDecisionSlug(value: string) {
  return /^[a-z0-9][a-z0-9-]*$/.test(value)
}
