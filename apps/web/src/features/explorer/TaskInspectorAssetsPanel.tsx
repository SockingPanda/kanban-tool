import { useCallback, useEffect, useId, useMemo, useRef, useState, type FormEvent } from "react"

import type { Locale } from "../../lib/preferences"
import type { TaskInspectorMutationSnapshot } from "./task-inspector-mutation-state"
import {
  createAttachmentUploadIntent,
  createInspectorAssetsActions,
  exactAttachmentBytes,
  formatAttachmentSize,
  advanceInspectorAssetsScope,
  isInspectorAssetsScopeCurrent,
  isAttachmentRetryDraftCurrent,
  requestSuggestedLabels,
  shouldClearAssetDraft,
  type InspectorAssetAttachment,
  type InspectorAssetLabel,
  type InspectorAssetsActions,
  type InspectorAssetsMutationHandlers,
  type InspectorLabelSuggestionResult,
  type InspectorAssetsScopeIdentity,
  type SuggestLabelsHandler,
} from "./TaskInspectorAssetsPanel.logic"
import styles from "./TaskInspectorAssetsPanel.module.css"

export type { InspectorAssetAttachment, InspectorAssetLabel, InspectorAssetsActions, InspectorAssetsMutationHandlers, InspectorAssetsScopeIdentity, InspectorLabelSuggestionResult, SuggestLabelsHandler }

export interface TaskInspectorAssetsPanelProps {
  readonly taskId: string
  readonly labels: readonly InspectorAssetLabel[]
  readonly attachments: readonly InspectorAssetAttachment[]
  readonly suggestionResult: InspectorLabelSuggestionResult | null
  readonly suggestionRequested: boolean
  readonly suggestionLoading?: boolean
  readonly suggestionError?: string | null
  readonly handlers: InspectorAssetsMutationHandlers
  readonly snapshot: TaskInspectorMutationSnapshot
  readonly attachmentLoading?: boolean
  readonly attachmentError?: string | null
  readonly locale?: Locale
}

type AssetsCopy = {
  readonly title: string
  readonly labels: string
  readonly attachments: string
  readonly noLabels: string
  readonly labelName: string
  readonly labelPlaceholder: string
  readonly addLabel: string
  readonly removeLabel: (name: string) => string
  readonly suggestLabels: string
  readonly refreshSuggestions: string
  readonly suggesting: string
  readonly selected: string
  readonly candidates: string
  readonly noSuggestions: string
  readonly score: (value: string) => string
  readonly coverage: (coverage: string, cosine: string, residual: string) => string
  readonly degraded: string
  readonly degradedDescription: string
  readonly reasonCodes: string
  readonly diagnostics: string
  readonly duplicateSuggestion: (ids: string) => string
  readonly applied: string
  readonly apply: string
  readonly chooseFile: string
  readonly upload: string
  readonly uploading: string
  readonly loadingAttachments: string
  readonly noAttachments: string
  readonly attachmentError: string
  readonly filename: string
  readonly contentType: string
  readonly size: string
  readonly sha256: string
  readonly createdBy: string
  readonly createdAt: string
  readonly download: (name: string) => string
  readonly deleteAttachment: (name: string) => string
  readonly downloading: string
  readonly deleting: string
  readonly mutationError: string
  readonly reloadStale: string
  readonly retry: string
  readonly retrying: string
  readonly error: string
}

const copies: Record<Locale, AssetsCopy> = {
  zh: {
    title: "标签与附件",
    labels: "标签",
    attachments: "附件",
    noLabels: "暂无标签。",
    labelName: "标签名称",
    labelPlaceholder: "输入标签名称…",
    addLabel: "添加标签",
    removeLabel: (name) => `移除标签 ${name}`,
    suggestLabels: "建议标签",
    refreshSuggestions: "刷新建议",
    suggesting: "正在获取建议…",
    selected: "已选建议",
    candidates: "候选标签",
    noSuggestions: "暂无标签建议。",
    score: (value) => `分数 ${value}`,
    coverage: (coverage, cosine, residual) => `覆盖率 ${coverage} / cosine ${cosine} / residual ${residual}`,
    degraded: "建议结果已降级",
    degradedDescription: "以下结果需要人工复核；降级结果不会被静默视为成功。",
    reasonCodes: "原因码",
    diagnostics: "诊断",
    duplicateSuggestion: (ids) => `建议结果包含重复 label_id：${ids}。请人工复核后再应用。`,
    applied: "已应用",
    apply: "应用",
    chooseFile: "选择附件文件",
    upload: "上传附件",
    uploading: "正在上传…",
    loadingAttachments: "正在加载附件…",
    noAttachments: "暂无附件。",
    attachmentError: "附件加载或操作失败",
    filename: "文件名",
    contentType: "类型",
    size: "大小",
    sha256: "SHA-256",
    createdBy: "上传者",
    createdAt: "创建时间",
    download: (name) => `下载附件 ${name}`,
    deleteAttachment: (name) => `删除附件 ${name}`,
    downloading: "正在下载…",
    deleting: "正在删除…",
    mutationError: "操作失败",
    reloadStale: "写入已提交，但刷新失败；当前数据可能过期。",
    retry: "重试原提交",
    retrying: "正在重试原提交…",
    error: "操作失败，请重试。",
  },
  en: {
    title: "Labels & attachments",
    labels: "Labels",
    attachments: "Attachments",
    noLabels: "No labels.",
    labelName: "Label name",
    labelPlaceholder: "Enter a label name…",
    addLabel: "Add label",
    removeLabel: (name) => `Remove label ${name}`,
    suggestLabels: "Suggest labels",
    refreshSuggestions: "Refresh suggestions",
    suggesting: "Finding suggestions…",
    selected: "Selected suggestions",
    candidates: "Candidates",
    noSuggestions: "No label suggestions.",
    score: (value) => `Score ${value}`,
    coverage: (coverage, cosine, residual) => `Coverage ${coverage} / cosine ${cosine} / residual ${residual}`,
    degraded: "Suggestions degraded",
    degradedDescription: "Review these results manually; degraded output is not silently treated as success.",
    reasonCodes: "Reason codes",
    diagnostics: "Diagnostics",
    duplicateSuggestion: (ids) => `Suggestions contain duplicate label_id values: ${ids}. Review before applying.`,
    applied: "Applied",
    apply: "Apply",
    chooseFile: "Choose attachment file",
    upload: "Upload attachment",
    uploading: "Uploading…",
    loadingAttachments: "Loading attachments…",
    noAttachments: "No attachments.",
    attachmentError: "Attachment load or operation failed",
    filename: "Filename",
    contentType: "Type",
    size: "Size",
    sha256: "SHA-256",
    createdBy: "Uploaded by",
    createdAt: "Created",
    download: (name) => `Download attachment ${name}`,
    deleteAttachment: (name) => `Delete attachment ${name}`,
    downloading: "Downloading…",
    deleting: "Deleting…",
    mutationError: "Operation failed",
    reloadStale: "The write committed, but refresh failed; data may be stale.",
    retry: "Retry original submission",
    retrying: "Retrying original submission…",
    error: "Operation failed. Try again.",
  },
}

function normalizedLabelName(value: string): string {
  return value.trim().toLowerCase()
}

function errorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) return error.message
  if (typeof error === "string" && error.trim()) return error
  return "操作失败，请重试。"
}

function actionKey(operation: string, taskId: string): string {
  return `${operation}:${taskId}`
}

function operationFromMutationKey(key: string, taskId: string): string | null {
  const suffix = `:${taskId}`
  return key.endsWith(suffix) ? key.slice(0, -suffix.length) : null
}

const writeOperations = [
  "saveTask",
  "transition",
  "addDependency",
  "removeDependency",
  "createStep",
  "linkStep",
  "markPlanNotRequired",
  "addLabel",
  "removeLabel",
  "applySuggestedLabel",
  "addComment",
  "uploadAttachment",
  "deleteAttachment",
] as const

function suggestionPercent(value: number): string {
  return Number.isFinite(value) ? `${(value * 100).toFixed(0)}%` : "—"
}

function suggestionResidual(value: number): string {
  return Number.isFinite(value) ? value.toFixed(3) : "—"
}

function attachmentTime(value: number, locale: Locale): { readonly iso: string; readonly display: string } {
  const milliseconds = Math.abs(value) < 1_000_000_000_000 ? value * 1_000 : value
  const date = new Date(milliseconds)
  if (Number.isNaN(date.getTime())) return { iso: String(value), display: String(value) }
  const language = locale === "en" ? "en-US" : "zh-CN"
  try {
    return {
      iso: date.toISOString(),
      display: new Intl.DateTimeFormat(language, { dateStyle: "short", timeStyle: "short" }).format(date),
    }
  } catch {
    return { iso: date.toISOString(), display: date.toISOString() }
  }
}

function Evidence({ entry }: { readonly entry: InspectorLabelSuggestionResult["selected_labels"][number] }) {
  const atoms = [...entry.evidence_atoms, ...entry.negative_evidence_atoms]
  if (atoms.length === 0) return <span className={styles.evidence}>—</span>
  return <>{atoms.map((atom, index) => <span key={`${atom.polarity}-${atom.atom_id}-${index}`} className={styles.evidence} title={atom.text}>{atom.text}</span>)}</>
}

function SuggestionRow({
  entry,
  copy,
  disabled,
  applied,
  onApply,
}: {
  readonly entry: InspectorLabelSuggestionResult["selected_labels"][number]
  readonly copy: AssetsCopy
  readonly disabled: boolean
  readonly applied: boolean
  readonly onApply: (name: string) => void
}) {
  return (
    <li className={styles.suggestionItem}>
      <div className={styles.suggestionBody}>
        <strong title={entry.label_name}>{entry.label_name}</strong>
        <span>{copy.score(entry.score.toFixed(3))}</span>
        <Evidence entry={entry} />
      </div>
      <button type="button" className={styles.smallButton} data-testid="label-suggestion-apply" disabled={disabled} aria-label={`${applied ? copy.applied : copy.apply} ${entry.label_name}`} onClick={() => onApply(entry.label_name)}>{applied ? copy.applied : copy.apply}</button>
    </li>
  )
}

function AttachmentMetadata({ attachment, copy, locale }: { readonly attachment: InspectorAssetAttachment; readonly copy: AssetsCopy; readonly locale: Locale }) {
  const createdAt = attachmentTime(attachment.created_at, locale)
  return (
    <dl className={styles.metadata}>
      <div><dt>{copy.filename}</dt><dd translate="no">{attachment.filename}</dd></div>
      <div><dt>{copy.contentType}</dt><dd translate="no">{attachment.content_type ?? "—"}</dd></div>
      <div><dt>{copy.size}</dt><dd>{formatAttachmentSize(attachment.size_bytes)}</dd></div>
      <div><dt>{copy.sha256}</dt><dd translate="no">{attachment.sha256 ?? "—"}</dd></div>
      <div><dt>{copy.createdBy}</dt><dd translate="no">{attachment.created_by}</dd></div>
      <div><dt>{copy.createdAt}</dt><dd><time dateTime={createdAt.iso}>{createdAt.display}</time></dd></div>
    </dl>
  )
}

export function TaskInspectorAssetsPanel({
  taskId,
  labels,
  attachments,
  suggestionResult,
  suggestionRequested,
  suggestionLoading = false,
  suggestionError = null,
  handlers,
  snapshot,
  attachmentLoading = false,
  attachmentError = null,
  locale = "zh",
}: TaskInspectorAssetsPanelProps) {
  const copy = copies[locale]
  const panelId = useId()
  const labelInputId = `${panelId}-label-name`
  const attachmentFileId = `${panelId}-attachment-file`
  const pending = snapshot.pending
  const errors = snapshot.errors
  const scopeIdentityRef = useRef<InspectorAssetsScopeIdentity>({ taskId, epoch: 0, generation: snapshot.generation })
  scopeIdentityRef.current = advanceInspectorAssetsScope(scopeIdentityRef.current, taskId, snapshot.generation)
  const isCurrentScope = useCallback((captured: InspectorAssetsScopeIdentity): boolean => isInspectorAssetsScopeCurrent(scopeIdentityRef.current, captured), [])
  const previousScopeRef = useRef({ taskId, generation: snapshot.generation })
  const localBusyRef = useRef(new Set<string>())
  const [localBusy, setLocalBusy] = useState<ReadonlySet<string>>(new Set())
  const [localErrors, setLocalErrors] = useState<ReadonlyMap<string, string>>(new Map())
  const [labelInput, setLabelInput] = useState("")
  const [selectedFile, setSelectedFile] = useState<File | null>(null)
  const labelInputRef = useRef(labelInput)
  labelInputRef.current = labelInput
  const selectedFileRef = useRef<File | null>(selectedFile)
  selectedFileRef.current = selectedFile
  const attemptedUploadFileRef = useRef<File | null>(null)
  const [fileInputKey, setFileInputKey] = useState(0)
  const [suggestionBusy, setSuggestionBusy] = useState(false)
  const [suggestionLocalError, setSuggestionLocalError] = useState<string | null>(null)
  const [suggestionLocalResult, setSuggestionLocalResult] = useState<InspectorLabelSuggestionResult | null>(null)
  const retryBusyRef = useRef(new Set<string>())
  const [retryBusy, setRetryBusy] = useState<ReadonlySet<string>>(new Set())

  useEffect(() => {
    const previous = previousScopeRef.current
    if (previous.taskId === taskId && previous.generation === snapshot.generation) return
    previousScopeRef.current = { taskId, generation: snapshot.generation }
    localBusyRef.current.clear()
    setLocalBusy(new Set())
    setLocalErrors(new Map())
    setLabelInput("")
    setSelectedFile(null)
    attemptedUploadFileRef.current = null
    setFileInputKey((current) => current + 1)
    setSuggestionBusy(false)
    setSuggestionLocalError(null)
    setSuggestionLocalResult(null)
    retryBusyRef.current.clear()
    setRetryBusy(new Set())
  }, [snapshot.generation, taskId])

  const isPending = useCallback((operation: string): boolean => {
    const key = actionKey(operation, taskId)
    return localBusy.has(key) || pending.has(key)
  }, [localBusy, pending, taskId])

  const errorFor = useCallback((operations: readonly string[]): string | null => {
    const keys = operations.map((operation) => actionKey(operation, taskId))
    for (const key of keys) {
      const local = localErrors.get(key)
      if (local) return local
    }
    return null
  }, [localErrors, taskId])

  const runAction = useCallback(async <T,>(operation: string, action: () => Promise<T>): Promise<T | null> => {
    const identity = scopeIdentityRef.current
    const key = actionKey(operation, identity.taskId)
    if (localBusyRef.current.has(key) || pending.has(key)) return null
    localBusyRef.current.add(key)
    setLocalBusy(new Set(localBusyRef.current))
    setLocalErrors((current) => {
      const next = new Map(current)
      next.delete(key)
      return next
    })
    try {
      const result = await action()
      return isCurrentScope(identity) ? result : null
    } catch (error) {
      if (isCurrentScope(identity)) setLocalErrors((current) => new Map(current).set(key, errorMessage(error)))
      return null
    } finally {
      if (isCurrentScope(identity)) {
        localBusyRef.current.delete(key)
        setLocalBusy(new Set(localBusyRef.current))
      }
    }
  }, [isCurrentScope, pending])

  const actions = useMemo(() => createInspectorAssetsActions(handlers), [handlers])
  const writePending = useMemo(
    () => writeOperations.some((operation) => isPending(operation)) || isPending("reload") || retryBusy.size > 0,
    [isPending, retryBusy],
  )
  const existingLabelNames = useMemo(() => new Set(labels.map((entry) => normalizedLabelName(entry.name))), [labels])
  const addLabel = useCallback(async () => {
    const name = labelInput.trim()
    if (!name || existingLabelNames.has(normalizedLabelName(name))) return
    const outcome = await runAction("addLabel", () => actions.addLabel(name))
    if (shouldClearAssetDraft(outcome) && labelInputRef.current.trim() === name) setLabelInput("")
  }, [actions, existingLabelNames, labelInput, runAction])
  const onLabelSubmit = useCallback((event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    void addLabel()
  }, [addLabel])
  const removeLabel = useCallback((labelId: string) => {
    void runAction("removeLabel", () => actions.removeLabel(labelId))
  }, [actions, runAction])

  const currentSuggestions = suggestionResult?.task_id === taskId
    ? suggestionResult
    : suggestionLocalResult?.task_id === taskId
      ? suggestionLocalResult
      : null
  const selectedSuggestions = useMemo(() => currentSuggestions?.selected_labels ?? [], [currentSuggestions])
  const candidateSuggestions = useMemo(() => currentSuggestions?.candidates ?? [], [currentSuggestions])
  const duplicateSuggestionIds = useMemo(() => {
    const counts = new Map<string, number>()
    for (const entry of [...selectedSuggestions, ...candidateSuggestions]) counts.set(entry.label_id, (counts.get(entry.label_id) ?? 0) + 1)
    return new Set([...counts.entries()].filter(([, count]) => count > 1).map(([id]) => id))
  }, [candidateSuggestions, selectedSuggestions])
  const applySuggestion = useCallback((labelName: string) => {
    if (existingLabelNames.has(normalizedLabelName(labelName))) return
    void runAction("applySuggestedLabel", () => actions.applySuggestedLabel(labelName))
  }, [actions, existingLabelNames, runAction])
  const suggestionPending = suggestionBusy || suggestionLoading || isPending("suggestLabels")
  const requestSuggestions = useCallback(() => {
    if (suggestionPending) return
    const identity = scopeIdentityRef.current
    setSuggestionLocalError(null)
    setSuggestionBusy(true)
    void requestSuggestedLabels(handlers.suggestLabels)
      .then((response) => {
        if (!isCurrentScope(identity)) return
        setSuggestionLocalResult(response?.data ?? null)
      })
      .catch((error: unknown) => {
        if (isCurrentScope(identity)) setSuggestionLocalError(errorMessage(error))
      })
      .finally(() => {
        if (isCurrentScope(identity)) setSuggestionBusy(false)
      })
  }, [handlers, isCurrentScope, suggestionPending])

  const uploadFile = useCallback(() => {
    if (!selectedFile) return
    const attemptedFile = selectedFile
    attemptedUploadFileRef.current = attemptedFile
    void runAction("uploadAttachment", async () => actions.uploadAttachment(await createAttachmentUploadIntent(attemptedFile)))
      .then((outcome) => {
        if (!shouldClearAssetDraft(outcome) || selectedFileRef.current !== attemptedFile) return
        setSelectedFile(null)
        attemptedUploadFileRef.current = null
        setFileInputKey((current) => current + 1)
      })
  }, [actions, runAction, selectedFile])

  const downloadAttachment = useCallback((attachment: InspectorAssetAttachment) => {
    void runAction("downloadAttachment", async () => {
      const downloaded = await actions.downloadAttachment(attachment.id)
      if (!downloaded || typeof document === "undefined" || typeof URL.createObjectURL !== "function") return
      const blob = new Blob([exactAttachmentBytes(downloaded.content)], { type: downloaded.content_type ?? attachment.content_type ?? "application/octet-stream" })
      const url = URL.createObjectURL(blob)
      const anchor = document.createElement("a")
      anchor.href = url
      anchor.download = attachment.filename
      anchor.click()
      anchor.remove()
      window.setTimeout(() => URL.revokeObjectURL(url), 0)
    })
  }, [actions, runAction])

  const deleteAttachment = useCallback((attachmentId: string) => {
    void runAction("deleteAttachment", () => actions.deleteAttachment(attachmentId))
  }, [actions, runAction])

  const retryMutation = useCallback((key: string) => {
    if (retryBusyRef.current.has(key) || pending.has(key) || writePending) return
    retryBusyRef.current.add(key)
    setRetryBusy(new Set(retryBusyRef.current))
    const identity = scopeIdentityRef.current
    const operation = operationFromMutationKey(key, identity.taskId)
    const intent = snapshot.retries.get(key)
    void Promise.resolve()
      .then(() => handlers.retry(key))
      .then(async (outcome) => {
        if (!isCurrentScope(identity) || !shouldClearAssetDraft(outcome)) return
        if (operation === "addLabel" && intent?.operation === "addLabel" && typeof intent.input.name === "string" && intent.input.name.trim() === labelInputRef.current.trim()) setLabelInput("")
        const currentFile = selectedFileRef.current
        if (operation === "uploadAttachment" && intent?.operation === "uploadAttachment" && isAttachmentRetryDraftCurrent(currentFile, attemptedUploadFileRef.current) && isCurrentScope(identity)) {
          setSelectedFile(null)
          attemptedUploadFileRef.current = null
          setFileInputKey((current) => current + 1)
        }
      })
      .catch(() => undefined)
      .finally(() => {
        if (!isCurrentScope(identity)) return
        retryBusyRef.current.delete(key)
        setRetryBusy(new Set(retryBusyRef.current))
      })
  }, [handlers, isCurrentScope, pending, snapshot.retries, writePending])

  const labelsError = errorFor(["addLabel", "removeLabel", "applySuggestedLabel"])
  const suggestionsError = suggestionLocalError ?? suggestionError ?? errorFor(["suggestLabels"])
  const attachmentsError = attachmentError ?? errorFor(["uploadAttachment", "downloadAttachment", "deleteAttachment"])
  const showSuggestions = suggestionRequested || Boolean(currentSuggestions) || suggestionPending || Boolean(suggestionsError)
  const duplicateIdsText = [...duplicateSuggestionIds].join(", ")
  const uploadPending = isPending("uploadAttachment")
  const snapshotErrors = useMemo(
    () => [...errors.entries()].filter(([key, error]) => error.taskId === taskId && key.endsWith(`:${taskId}`) && !pending.has(key) && !retryBusy.has(key)),
    [errors, pending, retryBusy, taskId],
  )
  const canRetrySnapshotError = useCallback((key: string, operation: string): boolean => {
    if (!snapshot.retries.has(key) || operation === "suggestLabels" || operation === "downloadAttachment") return false
    return true
  }, [snapshot.retries])
  const addLabelRetryIntent = snapshot.retries.get(actionKey("addLabel", taskId))
  const addLabelRetryLocked = addLabelRetryIntent?.operation === "addLabel"
    && typeof addLabelRetryIntent.input.name === "string"
    && addLabelRetryIntent.input.name.trim() === labelInput.trim()
  const uploadRetryIntent = snapshot.retries.get(actionKey("uploadAttachment", taskId))
  const uploadRetryLocked = uploadRetryIntent?.operation === "uploadAttachment"
    && selectedFile !== null
    && isAttachmentRetryDraftCurrent(selectedFile, attemptedUploadFileRef.current)
  const panelBusy = suggestionPending || writePending || isPending("downloadAttachment") || attachmentLoading
  const retrying = retryBusy.size > 0 || isPending("reload")

  return (
    <section className={styles.panel} data-testid="inspector-assets" aria-labelledby={`${panelId}-heading`} aria-busy={panelBusy}>
      <header className={styles.heading}>
        <p className={styles.kicker}>{copy.title}</p>
        <h2 id={`${panelId}-heading`}>{copy.title}</h2>
        <p className={styles.taskId} translate="no">{taskId}</p>
      </header>

      {snapshotErrors.length > 0 ? (
        <div className={styles.mutationErrors} data-testid="inspector-mutation-errors">
          {snapshotErrors.map(([key, error]) => (
            <div key={key} className={styles.mutationError} data-testid="inspector-mutation-error" data-operation-key={key} role="alert">
              <p><strong>{error.operation === "reload" ? copy.reloadStale : copy.mutationError}:</strong> {error.message}</p>
              {canRetrySnapshotError(key, error.operation) ? <button type="button" className={styles.smallButton} data-testid="inspector-retry" data-retry-key={key} disabled={retryBusy.has(key) || pending.has(key) || writePending} onClick={() => retryMutation(key)}>{retryBusy.has(key) ? "…" : copy.retry}</button> : null}
            </div>
          ))}
        </div>
      ) : null}
      {retrying ? <p className={styles.state} data-testid="inspector-retry-status" role="status" aria-live="polite">{copy.retrying}</p> : null}

      <section className={styles.section} data-testid="inspector-labels" aria-labelledby={`${panelId}-labels-heading`}>
        <h3 id={`${panelId}-labels-heading`}>{copy.labels}</h3>
        {labels.length > 0 ? (
          <ul className={styles.labelList}>
            {labels.map((label) => <li key={label.id} className={styles.labelChip}><span className={styles.labelName} title={label.name}>{label.name}</span><button type="button" className={styles.iconButton} data-testid="label-remove" disabled={writePending} aria-label={copy.removeLabel(label.name)} onClick={() => removeLabel(label.id)}>×</button></li>)}
          </ul>
        ) : <p className={styles.empty} data-testid="labels-empty" role="status">{copy.noLabels}</p>}
        <form className={styles.labelForm} onSubmit={onLabelSubmit}>
          <label htmlFor={labelInputId}>{copy.labelName}</label>
          <div className={styles.inputRow}>
            <input id={labelInputId} name="label-name" autoComplete="off" value={labelInput} placeholder={copy.labelPlaceholder} disabled={writePending || addLabelRetryLocked} onChange={(event) => setLabelInput(event.currentTarget.value)} />
            <button type="submit" className={styles.actionButton} data-testid="label-add" disabled={!labelInput.trim() || existingLabelNames.has(normalizedLabelName(labelInput)) || writePending || addLabelRetryLocked}>{isPending("addLabel") ? "…" : copy.addLabel}</button>
          </div>
        </form>
        {labelsError ? <p className={styles.error} role="alert">{labelsError}</p> : null}
        <div className={styles.suggestionActions}>
          <button type="button" className={styles.actionButton} data-testid="label-suggestion-request" disabled={suggestionPending} onClick={requestSuggestions}>{suggestionPending ? copy.suggesting : suggestionRequested || currentSuggestions ? copy.refreshSuggestions : copy.suggestLabels}</button>
        </div>
        {showSuggestions ? (
          <div className={styles.suggestionPanel} data-testid="label-suggestions">
            {currentSuggestions ? <p className={styles.metrics}>{copy.coverage(suggestionPercent(currentSuggestions.coverage), suggestionPercent(currentSuggestions.coverage_cosine), suggestionResidual(currentSuggestions.residual_norm))}</p> : null}
            {suggestionPending && !currentSuggestions ? <p className={styles.state} role="status" aria-live="polite">{copy.suggesting}</p> : null}
            {suggestionsError ? <p className={styles.error} role="alert">{suggestionsError}</p> : null}
            {currentSuggestions ? (
              <div className={styles.provenance} data-testid="label-suggestion-provenance">
                <p><span className={styles.metaLabel}>{copy.reasonCodes}:</span> {currentSuggestions.reason_codes.length ? currentSuggestions.reason_codes.join(", ") : "—"}</p>
                <p><span className={styles.metaLabel}>{copy.diagnostics}:</span> {currentSuggestions.diagnostics.length ? currentSuggestions.diagnostics.join(", ") : "—"}</p>
              </div>
            ) : null}
            {currentSuggestions?.degraded ? <div className={styles.degraded} role="alert"><strong>{copy.degraded}</strong><p>{copy.degradedDescription}</p></div> : null}
            {duplicateSuggestionIds.size > 0 ? <p className={styles.error} data-testid="label-suggestion-duplicates" role="alert">{copy.duplicateSuggestion(duplicateIdsText)}</p> : null}
            {selectedSuggestions.length > 0 ? <div className={styles.suggestionGroup}><h4>{copy.selected}</h4><ul className={styles.suggestionList}>{selectedSuggestions.map((entry) => <SuggestionRow key={`selected-${entry.label_id}-${entry.label_name}`} entry={entry} copy={copy} applied={entry.already_applied || existingLabelNames.has(normalizedLabelName(entry.label_name))} disabled={writePending || entry.already_applied || duplicateSuggestionIds.has(entry.label_id) || existingLabelNames.has(normalizedLabelName(entry.label_name))} onApply={applySuggestion} />)}</ul></div> : null}
            {candidateSuggestions.length > 0 ? <div className={styles.suggestionGroup}><h4>{copy.candidates}</h4><ul className={styles.suggestionList}>{candidateSuggestions.map((entry) => <SuggestionRow key={`candidate-${entry.label_id}-${entry.label_name}`} entry={entry} copy={copy} applied={entry.already_applied || existingLabelNames.has(normalizedLabelName(entry.label_name))} disabled={writePending || entry.already_applied || duplicateSuggestionIds.has(entry.label_id) || existingLabelNames.has(normalizedLabelName(entry.label_name))} onApply={applySuggestion} />)}</ul></div> : null}
            {!suggestionPending && !suggestionsError && currentSuggestions && selectedSuggestions.length === 0 && candidateSuggestions.length === 0 ? <p className={styles.empty} role="status">{copy.noSuggestions}</p> : null}
          </div>
        ) : null}
      </section>

      <section className={styles.section} data-testid="inspector-attachments" aria-labelledby={`${panelId}-attachments-heading`}>
        <h3 id={`${panelId}-attachments-heading`}>{copy.attachments}</h3>
        <div className={styles.uploadBox}>
          <label htmlFor={attachmentFileId}>{copy.chooseFile}</label>
          <input id={attachmentFileId} data-testid="attachment-file" key={fileInputKey} type="file" aria-label={copy.chooseFile} disabled={writePending} onChange={(event) => setSelectedFile(event.currentTarget.files?.[0] ?? null)} />
          <button type="button" className={styles.actionButton} data-testid="attachment-upload" disabled={!selectedFile || writePending || uploadRetryLocked} onClick={uploadFile}>{uploadPending ? copy.uploading : copy.upload}</button>
        </div>
        {attachmentLoading ? <p className={styles.state} data-testid="attachments-loading" role="status" aria-live="polite">{copy.loadingAttachments}</p> : null}
        {attachmentsError ? <p className={styles.error} data-testid="attachments-error" role="alert"><strong>{copy.attachmentError}:</strong> {attachmentsError}</p> : null}
        {attachments.length > 0 ? <ul className={styles.attachmentList}>{attachments.map((attachment) => <li key={attachment.id} className={styles.attachmentItem} data-testid="attachment-row"><div className={styles.attachmentMain}><strong className={styles.attachmentName} title={attachment.filename} translate="no">{attachment.filename}</strong><AttachmentMetadata attachment={attachment} copy={copy} locale={locale} /></div><div className={styles.attachmentActions}><button type="button" className={styles.smallButton} data-testid="attachment-download" disabled={isPending("downloadAttachment")} aria-label={copy.download(attachment.filename)} onClick={() => downloadAttachment(attachment)}>{isPending("downloadAttachment") ? copy.downloading : copy.download(attachment.filename)}</button><button type="button" className={styles.dangerButton} data-testid="attachment-delete" disabled={writePending} aria-label={copy.deleteAttachment(attachment.filename)} onClick={() => deleteAttachment(attachment.id)}>{isPending("deleteAttachment") ? copy.deleting : copy.deleteAttachment(attachment.filename)}</button></div></li>)}</ul> : <p className={styles.empty} data-testid="attachments-empty" role="status">{copy.noAttachments}</p>}
      </section>
    </section>
  )
}
