import { useCallback, useMemo, useRef, useState, type FormEvent } from "react"

import type { TaskInspectorMutationError, TaskInspectorMutationSnapshot } from "./task-inspector-mutation-state"
import type { Locale } from "../../lib/preferences"
import {
  createAttachmentUploadIntent,
  createInspectorAssetsActions,
  formatAttachmentSize,
  type InspectorAssetAttachment,
  type InspectorAssetLabel,
  type InspectorAssetsActions,
  type InspectorAssetsMutationHandlers,
  type InspectorLabelSuggestionResult,
  type SuggestLabelsHandler,
} from "./TaskInspectorAssetsPanel.logic"
import styles from "./TaskInspectorAssetsPanel.module.css"

export type { InspectorAssetAttachment, InspectorAssetLabel, InspectorAssetsActions, InspectorAssetsMutationHandlers, InspectorLabelSuggestionResult, SuggestLabelsHandler }

export interface InspectorLabelSuggestionState {
  readonly result: InspectorLabelSuggestionResult | null
  readonly requested: boolean
  readonly loading: boolean
  readonly error: string | null
}

export interface TaskInspectorAssetsPanelProps {
  readonly taskId: string
  readonly labels?: readonly InspectorAssetLabel[]
  readonly attachments?: readonly InspectorAssetAttachment[]
  /** Compatibility aliases for callers that keep canonical snapshots explicitly named. */
  readonly currentLabels?: readonly InspectorAssetLabel[]
  readonly currentAttachments?: readonly InspectorAssetAttachment[]
  readonly labelSuggestions?: InspectorLabelSuggestionResult | null
  readonly suggestionResult?: InspectorLabelSuggestionResult | null
  readonly labelSuggestionsRequested?: boolean
  readonly suggestionRequested?: boolean
  readonly labelSuggestionsLoading?: boolean
  readonly suggestionLoading?: boolean
  readonly labelSuggestionsError?: string | null
  readonly suggestionError?: string | null
  readonly suggestionState?: InspectorLabelSuggestionState
  /** The controller-owned typed handler. This is called only by the explicit button. */
  readonly suggestLabels?: SuggestLabelsHandler
  /** Kept as a narrow compatibility seam while the 05C handler settles. */
  readonly onSuggestLabels?: SuggestLabelsHandler
  readonly handlers?: InspectorAssetsMutationHandlers
  readonly mutationHandlers?: InspectorAssetsMutationHandlers
  readonly onAddLabel?: InspectorAssetsMutationHandlers["addLabel"]
  readonly onRemoveLabel?: InspectorAssetsMutationHandlers["removeLabel"]
  readonly onApplySuggestedLabel?: InspectorAssetsMutationHandlers["applySuggestedLabel"]
  readonly onUploadAttachment?: InspectorAssetsMutationHandlers["uploadAttachment"]
  readonly onDownloadAttachment?: InspectorAssetsMutationHandlers["downloadAttachment"]
  readonly onDeleteAttachment?: InspectorAssetsMutationHandlers["deleteAttachment"]
  readonly snapshot?: TaskInspectorMutationSnapshot
  readonly mutationSnapshot?: TaskInspectorMutationSnapshot
  readonly attachmentLoading?: boolean
  readonly attachmentsLoading?: boolean
  readonly attachmentError?: string | null
  readonly attachmentsError?: string | null
  readonly locale?: Locale
}

/** Host binary response and attachment write budget; keep in step with kanban-server. */
const EMPTY_PENDING = new Set<string>()
const EMPTY_ERRORS = new Map<string, TaskInspectorMutationError>()

function normalizedLabelName(value: string): string {
  return value.trim().toLowerCase()
}

function errorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) return error.message
  if (typeof error === "string" && error.trim()) return error
  return "操作失败，请重试。"
}

function mutationError(errors: ReadonlyMap<string, TaskInspectorMutationError>, keys: readonly string[]): string | null {
  for (const key of keys) {
    const error = errors.get(key)
    if (error) return error.message
  }
  return null
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
  readonly negativeEvidence: (count: number) => string
  readonly coverage: (coverage: string, cosine: string, residual: string) => string
  readonly degraded: string
  readonly degradedDescription: string
  readonly reasonCodes: string
  readonly diagnostics: string
  readonly applied: string
  readonly apply: string
  readonly chooseFile: string
  readonly upload: string
  readonly uploading: string
  readonly loadingAttachments: string
  readonly noAttachments: string
  readonly attachmentError: string
  readonly contentType: string
  readonly size: string
  readonly sha256: string
  readonly createdBy: string
  readonly download: (name: string) => string
  readonly deleteAttachment: (name: string) => string
  readonly downloading: string
  readonly deleting: string
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
    negativeEvidence: (count) => `负面证据 ${count} 条`,
    coverage: (coverage, cosine, residual) => `覆盖率 ${coverage} / cosine ${cosine} / residual ${residual}`,
    degraded: "建议结果已降级",
    degradedDescription: "以下结果需要人工复核；降级结果不会被静默视为成功。",
    reasonCodes: "原因码",
    diagnostics: "诊断",
    applied: "已应用",
    apply: "应用",
    chooseFile: "选择附件文件",
    upload: "上传附件",
    uploading: "正在上传…",
    loadingAttachments: "正在加载附件…",
    noAttachments: "暂无附件。",
    attachmentError: "附件加载或操作失败",
    contentType: "类型",
    size: "大小",
    sha256: "SHA-256",
    createdBy: "上传者",
    download: (name) => `下载附件 ${name}`,
    deleteAttachment: (name) => `删除附件 ${name}`,
    downloading: "正在下载…",
    deleting: "正在删除…",
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
    negativeEvidence: (count) => `Negative evidence ${count}`,
    coverage: (coverage, cosine, residual) => `Coverage ${coverage} / cosine ${cosine} / residual ${residual}`,
    degraded: "Suggestions degraded",
    degradedDescription: "Review these results manually; degraded output is not silently treated as success.",
    reasonCodes: "Reason codes",
    diagnostics: "Diagnostics",
    applied: "Applied",
    apply: "Apply",
    chooseFile: "Choose attachment file",
    upload: "Upload attachment",
    uploading: "Uploading…",
    loadingAttachments: "Loading attachments…",
    noAttachments: "No attachments.",
    attachmentError: "Attachment load or operation failed",
    contentType: "Type",
    size: "Size",
    sha256: "SHA-256",
    createdBy: "Uploaded by",
    download: (name) => `Download attachment ${name}`,
    deleteAttachment: (name) => `Delete attachment ${name}`,
    downloading: "Downloading…",
    deleting: "Deleting…",
    error: "Operation failed. Try again.",
  },
}

function suggestionPercent(value: number): string {
  return Number.isFinite(value) ? `${(value * 100).toFixed(0)}%` : "—"
}

function suggestionResidual(value: number): string {
  return Number.isFinite(value) ? value.toFixed(3) : "—"
}

function actionKey(operation: string, taskId: string): string {
  return `${operation}:${taskId}`
}

function taskErrorMessage(error: unknown, fallback: string): string {
  const message = errorMessage(error)
  return message === "操作失败，请重试。" ? fallback : message
}

function AttachmentMetadata({ attachment, copy }: { readonly attachment: InspectorAssetAttachment; readonly copy: AssetsCopy }) {
  return (
    <dl className={styles.metadata}>
      <div><dt>{copy.contentType}</dt><dd translate="no">{attachment.content_type ?? "—"}</dd></div>
      <div><dt>{copy.size}</dt><dd>{formatAttachmentSize(attachment.size_bytes)}</dd></div>
      <div><dt>{copy.sha256}</dt><dd translate="no">{attachment.sha256 ?? "—"}</dd></div>
      <div><dt>{copy.createdBy}</dt><dd translate="no">{attachment.created_by}</dd></div>
    </dl>
  )
}

export function TaskInspectorAssetsPanel({
  taskId,
  labels,
  attachments,
  currentLabels,
  currentAttachments,
  labelSuggestions,
  suggestionResult,
  labelSuggestionsRequested,
  suggestionRequested,
  labelSuggestionsLoading,
  suggestionLoading,
  labelSuggestionsError,
  suggestionError,
  suggestionState,
  suggestLabels,
  onSuggestLabels,
  handlers,
  mutationHandlers,
  onAddLabel,
  onRemoveLabel,
  onApplySuggestedLabel,
  onUploadAttachment,
  onDownloadAttachment,
  onDeleteAttachment,
  snapshot,
  mutationSnapshot,
  attachmentLoading,
  attachmentsLoading,
  attachmentError,
  attachmentsError: attachmentsErrorProp,
  locale = "zh",
}: TaskInspectorAssetsPanelProps) {
  const copy = copies[locale]
  const currentLabelList = useMemo(() => labels ?? currentLabels ?? [], [currentLabels, labels])
  const attachmentList = attachments ?? currentAttachments ?? []
  const mutation = handlers ?? mutationHandlers
  const currentSuggestions = suggestionState?.result ?? labelSuggestions ?? suggestionResult ?? null
  const requested = suggestionState?.requested ?? labelSuggestionsRequested ?? suggestionRequested ?? Boolean(currentSuggestions)
  const hostSuggestionLoading = suggestionState?.loading ?? labelSuggestionsLoading ?? suggestionLoading ?? false
  const hostSuggestionError = suggestionState?.error ?? labelSuggestionsError ?? suggestionError ?? null
  const activeSnapshot = snapshot ?? mutationSnapshot
  const pending = activeSnapshot?.pending ?? EMPTY_PENDING
  const errors = activeSnapshot?.errors ?? EMPTY_ERRORS
  const localBusyRef = useRef(new Set<string>())
  const [localBusy, setLocalBusy] = useState<ReadonlySet<string>>(EMPTY_PENDING)
  const [localErrors, setLocalErrors] = useState<ReadonlyMap<string, string>>(new Map())
  const [labelInput, setLabelInput] = useState("")
  const [selectedFile, setSelectedFile] = useState<File | null>(null)
  const [fileInputKey, setFileInputKey] = useState(0)
  const [suggestionBusy, setSuggestionBusy] = useState(false)
  const [suggestionLocalError, setSuggestionLocalError] = useState<string | null>(null)

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
    return mutationError(errors, keys)
  }, [errors, localErrors, taskId])

  const runAction = useCallback(async (operation: string, action: () => Promise<unknown>): Promise<boolean> => {
    const key = actionKey(operation, taskId)
    if (localBusyRef.current.has(key) || pending.has(key)) return false
    localBusyRef.current.add(key)
    setLocalBusy(new Set(localBusyRef.current))
    setLocalErrors((current) => {
      const next = new Map(current)
      next.delete(key)
      return next
    })
    try {
      await action()
      return true
    } catch (error) {
      setLocalErrors((current) => new Map(current).set(key, errorMessage(error)))
      return false
    } finally {
      localBusyRef.current.delete(key)
      setLocalBusy(new Set(localBusyRef.current))
    }
  }, [pending, taskId])

  const resolvedHandlers: InspectorAssetsMutationHandlers = useMemo(() => ({
    addLabel: mutation?.addLabel ?? onAddLabel ?? (async () => undefined),
    removeLabel: mutation?.removeLabel ?? onRemoveLabel ?? (async () => undefined),
    applySuggestedLabel: mutation?.applySuggestedLabel ?? onApplySuggestedLabel ?? (async () => undefined),
    uploadAttachment: mutation?.uploadAttachment ?? onUploadAttachment ?? (async () => undefined),
    downloadAttachment: mutation?.downloadAttachment ?? onDownloadAttachment ?? (async () => null),
    deleteAttachment: mutation?.deleteAttachment ?? onDeleteAttachment ?? (async () => undefined),
  }), [mutation, onAddLabel, onApplySuggestedLabel, onDeleteAttachment, onDownloadAttachment, onRemoveLabel, onUploadAttachment])
  const actions = useMemo(() => createInspectorAssetsActions(resolvedHandlers), [resolvedHandlers])

  const existingLabelNames = useMemo(
    () => new Set(currentLabelList.map((entry) => normalizedLabelName(entry.name))),
    [currentLabelList],
  )
  const addLabel = useCallback(async () => {
    const name = labelInput.trim()
    if (!name || existingLabelNames.has(normalizedLabelName(name))) return
    const succeeded = await runAction("addLabel", () => actions.addLabel(name))
    if (succeeded) setLabelInput("")
  }, [actions, existingLabelNames, labelInput, runAction])

  const onLabelSubmit = useCallback((event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    void addLabel()
  }, [addLabel])

  const removeLabel = useCallback((labelId: string) => {
    void runAction("removeLabel", () => actions.removeLabel(labelId))
  }, [actions, runAction])

  const applySuggestion = useCallback((labelName: string) => {
    if (existingLabelNames.has(normalizedLabelName(labelName))) return
    void runAction("applySuggestedLabel", () => actions.applySuggestedLabel(labelName))
  }, [actions, existingLabelNames, runAction])

  const chooseFile = useCallback((file: File | null) => {
    setSelectedFile(file)
    setLocalErrors((current) => {
      const next = new Map(current)
      next.delete(actionKey("uploadAttachment", taskId))
      return next
    })
  }, [taskId])

  const uploadFile = useCallback(() => {
    if (!selectedFile) return
    void runAction("uploadAttachment", async () => {
      await actions.uploadAttachment(await createAttachmentUploadIntent(selectedFile))
      setSelectedFile(null)
      setFileInputKey((current) => current + 1)
    })
  }, [actions, runAction, selectedFile])

  const downloadAttachment = useCallback((attachment: InspectorAssetAttachment) => {
    void runAction("downloadAttachment", async () => {
      const downloaded = await actions.downloadAttachment(attachment.id)
      if (!downloaded || typeof document === "undefined" || typeof URL.createObjectURL !== "function") return
      const blob = new Blob([downloaded.content.buffer as ArrayBuffer], { type: downloaded.content_type ?? attachment.content_type ?? "application/octet-stream" })
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

  const suggestionHandler = suggestLabels ?? onSuggestLabels ?? mutation?.suggestLabels
  const suggestionIsPending = suggestionBusy || hostSuggestionLoading || isPending("suggestLabels") || isPending("suggestTaskLabels")
  const requestSuggestions = useCallback(() => {
    if (!suggestionHandler || suggestionIsPending) return
    setSuggestionLocalError(null)
    setSuggestionBusy(true)
    void Promise.resolve(suggestionHandler({ limit: 5 })).catch((error: unknown) => {
      setSuggestionLocalError(taskErrorMessage(error, copy.error))
    }).finally(() => setSuggestionBusy(false))
  }, [copy.error, suggestionHandler, suggestionIsPending])

  const suggestionErrorText = suggestionLocalError ?? hostSuggestionError ?? errorFor(["suggestLabels", "suggestTaskLabels"])
  const showSuggestions = requested || Boolean(currentSuggestions) || suggestionIsPending || Boolean(suggestionErrorText)
  const selectedSuggestions = useMemo(() => currentSuggestions?.selected_labels ?? [], [currentSuggestions])
  const selectedIds = useMemo(() => new Set(selectedSuggestions.map((entry) => entry.label_id)), [selectedSuggestions])
  const candidateSuggestions = currentSuggestions?.candidates.filter((entry) => !selectedIds.has(entry.label_id)) ?? []
  const labelsError = errorFor(["addLabel", "removeLabel", "applySuggestedLabel"])
  const uploadPending = isPending("uploadAttachment")
  const attachmentsError = attachmentsErrorProp ?? attachmentError ?? errorFor(["uploadAttachment", "downloadAttachment", "deleteAttachment"])
  const listLoading = attachmentsLoading ?? attachmentLoading ?? false

  return (
    <section className={styles.panel} data-testid="inspector-assets" aria-labelledby="inspector-assets-heading">
      <header className={styles.heading}>
        <p className={styles.kicker}>{copy.title}</p>
        <h2 id="inspector-assets-heading">{copy.title}</h2>
        <p className={styles.taskId} translate="no">{taskId}</p>
      </header>

      <section className={styles.section} data-testid="inspector-labels" aria-labelledby="inspector-labels-heading">
        <h3 id="inspector-labels-heading">{copy.labels}</h3>
        {currentLabelList.length > 0 ? (
          <ul className={styles.labelList}>
            {currentLabelList.map((label) => {
              const pendingRemove = isPending("removeLabel")
              return (
                <li key={label.id} className={styles.labelChip}>
                  <span className={styles.labelName} title={label.name}>{label.name}</span>
                  <button type="button" className={styles.iconButton} data-testid="label-remove" disabled={pendingRemove} aria-label={copy.removeLabel(label.name)} onClick={() => removeLabel(label.id)}>×</button>
                </li>
              )
            })}
          </ul>
        ) : <p className={styles.empty} data-testid="labels-empty" role="status">{copy.noLabels}</p>}
        <form className={styles.labelForm} onSubmit={onLabelSubmit}>
          <label htmlFor="inspector-label-name">{copy.labelName}</label>
          <div className={styles.inputRow}>
            <input
              id="inspector-label-name"
              name="label-name"
              autoComplete="off"
              value={labelInput}
              placeholder={copy.labelPlaceholder}
              onChange={(event) => setLabelInput(event.currentTarget.value)}
            />
            <button type="submit" className={styles.actionButton} data-testid="label-add" disabled={!labelInput.trim() || existingLabelNames.has(normalizedLabelName(labelInput)) || isPending("addLabel")}>
              {isPending("addLabel") ? "…" : copy.addLabel}
            </button>
          </div>
        </form>
        {labelsError ? <p className={styles.error} role="alert" aria-live="polite">{labelsError}</p> : null}

        <div className={styles.suggestionActions}>
          <button type="button" className={styles.actionButton} data-testid="label-suggestion-request" disabled={!suggestionHandler || suggestionIsPending} onClick={requestSuggestions}>
            {suggestionIsPending ? copy.suggesting : requested || currentSuggestions ? copy.refreshSuggestions : copy.suggestLabels}
          </button>
        </div>
        {showSuggestions ? (
          <div className={styles.suggestionPanel} data-testid="label-suggestions">
            {currentSuggestions ? (
              <p className={styles.metrics}>{copy.coverage(suggestionPercent(currentSuggestions.coverage), suggestionPercent(currentSuggestions.coverage_cosine), suggestionResidual(currentSuggestions.residual_norm))}</p>
            ) : null}
            {suggestionIsPending && !currentSuggestions ? <p className={styles.state} role="status">{copy.suggesting}</p> : null}
            {suggestionErrorText ? <p className={styles.error} role="alert" aria-live="polite">{suggestionErrorText}</p> : null}
            {currentSuggestions?.degraded ? (
              <div className={styles.degraded} role="alert" aria-live="polite">
                <strong>{copy.degraded}</strong>
                <p>{copy.degradedDescription}</p>
              </div>
            ) : null}
            {currentSuggestions && (currentSuggestions.reason_codes.length > 0 || currentSuggestions.diagnostics.length > 0) ? (
              <div className={styles.provenance} data-testid="label-suggestion-provenance">
                <p><span className={styles.metaLabel}>{copy.reasonCodes}:</span> {currentSuggestions.reason_codes.length ? currentSuggestions.reason_codes.join(", ") : "—"}</p>
                <p><span className={styles.metaLabel}>{copy.diagnostics}:</span> {currentSuggestions.diagnostics.length ? currentSuggestions.diagnostics.join(", ") : "—"}</p>
              </div>
            ) : null}
            {selectedSuggestions.length > 0 ? (
              <div className={styles.suggestionGroup}>
                <h4>{copy.selected}</h4>
                <ul className={styles.suggestionList}>
                  {selectedSuggestions.map((entry) => {
                    const disabled = entry.already_applied || existingLabelNames.has(normalizedLabelName(entry.label_name)) || isPending("applySuggestedLabel")
                    return (
                      <li key={`selected-${entry.label_id}`} className={styles.suggestionItem}>
                        <div className={styles.suggestionBody}>
                          <strong title={entry.label_name}>{entry.label_name}</strong>
                          <span>{copy.score(entry.score.toFixed(3))}</span>
                          {entry.evidence_atoms.slice(0, 2).map((atom) => <span key={atom.atom_id} className={styles.evidence} title={atom.text}>{atom.text}</span>)}
                          {entry.negative_evidence_atoms.length > 0 ? <span>{copy.negativeEvidence(entry.negative_evidence_atoms.length)}</span> : null}
                        </div>
                        <button type="button" className={styles.smallButton} data-testid="label-suggestion-apply" disabled={disabled} onClick={() => applySuggestion(entry.label_name)}>{entry.already_applied || existingLabelNames.has(normalizedLabelName(entry.label_name)) ? copy.applied : copy.apply}</button>
                      </li>
                    )
                  })}
                </ul>
              </div>
            ) : null}
            {candidateSuggestions.length > 0 ? (
              <div className={styles.suggestionGroup}>
                <h4>{copy.candidates}</h4>
                <ul className={styles.suggestionList}>
                  {candidateSuggestions.map((entry) => {
                    const disabled = entry.already_applied || existingLabelNames.has(normalizedLabelName(entry.label_name)) || isPending("applySuggestedLabel")
                    return (
                      <li key={`candidate-${entry.label_id}`} className={styles.suggestionItem}>
                        <div className={styles.suggestionBody}>
                          <strong title={entry.label_name}>{entry.label_name}</strong>
                          <span>{copy.score(entry.score.toFixed(3))}</span>
                          {entry.evidence_atoms.slice(0, 2).map((atom) => <span key={atom.atom_id} className={styles.evidence} title={atom.text}>{atom.text}</span>)}
                          {entry.negative_evidence_atoms.length > 0 ? <span>{copy.negativeEvidence(entry.negative_evidence_atoms.length)}</span> : null}
                        </div>
                        <button type="button" className={styles.smallButton} data-testid="label-suggestion-apply" disabled={disabled} onClick={() => applySuggestion(entry.label_name)}>{disabled ? copy.applied : copy.apply}</button>
                      </li>
                    )
                  })}
                </ul>
              </div>
            ) : null}
            {!suggestionIsPending && !suggestionErrorText && currentSuggestions && selectedSuggestions.length === 0 && candidateSuggestions.length === 0 ? <p className={styles.empty} role="status">{copy.noSuggestions}</p> : null}
          </div>
        ) : null}
      </section>

      <section className={styles.section} data-testid="inspector-attachments" aria-labelledby="inspector-attachments-heading">
        <h3 id="inspector-attachments-heading">{copy.attachments}</h3>
        <div className={styles.uploadBox}>
          <label htmlFor={`inspector-attachment-file-${taskId}`}>{copy.chooseFile}</label>
          <input id={`inspector-attachment-file-${taskId}`} data-testid="attachment-file" key={fileInputKey} type="file" aria-label={copy.chooseFile} disabled={uploadPending} onChange={(event) => chooseFile(event.currentTarget.files?.[0] ?? null)} />
          <button type="button" className={styles.actionButton} data-testid="attachment-upload" disabled={!selectedFile || uploadPending} onClick={uploadFile}>{uploadPending ? copy.uploading : copy.upload}</button>
        </div>
        {listLoading ? <p className={styles.state} data-testid="attachments-loading" role="status" aria-live="polite">{copy.loadingAttachments}</p> : null}
        {attachmentsError ? <p className={styles.error} data-testid="attachments-error" role="alert" aria-live="polite"><strong>{copy.attachmentError}:</strong> {attachmentsError}</p> : null}
        {attachmentList.length > 0 ? (
          <ul className={styles.attachmentList}>
            {attachmentList.map((attachment) => {
              const downloading = isPending("downloadAttachment")
              const deleting = isPending("deleteAttachment")
              return (
                <li key={attachment.id} className={styles.attachmentItem} data-testid="attachment-row">
                  <div className={styles.attachmentMain}>
                    <strong className={styles.attachmentName} title={attachment.filename} translate="no">{attachment.filename}</strong>
                    <AttachmentMetadata attachment={attachment} copy={copy} />
                  </div>
                  <div className={styles.attachmentActions}>
                    <button type="button" className={styles.smallButton} data-testid="attachment-download" disabled={downloading} aria-label={copy.download(attachment.filename)} onClick={() => downloadAttachment(attachment)}>{downloading ? copy.downloading : copy.download(attachment.filename)}</button>
                    <button type="button" className={styles.dangerButton} data-testid="attachment-delete" disabled={deleting} aria-label={copy.deleteAttachment(attachment.filename)} onClick={() => deleteAttachment(attachment.id)}>{deleting ? copy.deleting : copy.deleteAttachment(attachment.filename)}</button>
                  </div>
                </li>
              )
            })}
          </ul>
        ) : <p className={styles.empty} data-testid="attachments-empty" role="status">{copy.noAttachments}</p>}
      </section>
    </section>
  )
}
