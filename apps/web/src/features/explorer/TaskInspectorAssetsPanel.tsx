import { useCallback, useId, useLayoutEffect, useMemo, useRef, useState, type FormEvent } from "react"
import { Banner } from "@astryxdesign/core/Banner"
import { Button } from "@astryxdesign/core/Button"
import { Heading } from "@astryxdesign/core/Heading"
import { List, ListItem } from "@astryxdesign/core/List"
import { Text } from "@astryxdesign/core/Text"

import type { Locale } from "../../lib/preferences"
import type { TaskInspectorMutationSnapshot } from "./task-inspector-mutation-state"
import {
  FileInput,
  SafeHStack,
  SafeMetadataList,
  SafeMetadataListItem,
  SafeVStack,
  TextInput,
} from "@/ui/astryx"
import {
  createAttachmentUploadIntent,
  createInspectorAssetsActions,
  exactAttachmentBytes,
  formatAttachmentSize,
  MAX_ATTACHMENT_UPLOAD_BYTES,
  advanceInspectorAssetsScope,
  isInspectorAssetsScopeCurrent,
  isAttachmentRetryDraftCurrent,
  isLabelRetryDraftCurrent,
  requestSuggestedLabels,
  shouldShowInspectorSnapshotError,
  shouldClearAssetDraft,
  type InspectorAssetAttachment,
  type InspectorAssetLabel,
  type InspectorAssetsActions,
  type InspectorAssetsMutationHandlers,
  type InspectorLabelSuggestionResult,
  type InspectorAssetsScopeIdentity,
  type SuggestLabelsHandler,
} from "./TaskInspectorAssetsPanel.logic"

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
  readonly addingLabel: string
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
  readonly downloadError: string
  readonly mutationError: string
  readonly reloadStale: string
  readonly retry: string
  readonly retrying: string
  readonly error: string
  readonly clearFile: string
  readonly invalidFileType: string
  readonly fileTooLarge: (size: string) => string
  readonly fileCountExceeded: string
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
    addingLabel: "正在添加标签…",
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
    downloadError: "下载附件失败，请重试。",
    mutationError: "操作失败",
    reloadStale: "写入已提交，但刷新失败；当前数据可能过期。",
    retry: "重试原提交",
    retrying: "正在重试原提交…",
    error: "操作失败，请重试。",
    clearFile: "清除已选文件",
    invalidFileType: "文件类型不受支持。",
    fileTooLarge: (size) => `文件超过 ${size} 上传上限。`,
    fileCountExceeded: "一次只能选择一个文件。",
  },
  en: {
    title: "Labels & attachments",
    labels: "Labels",
    attachments: "Attachments",
    noLabels: "No labels.",
    labelName: "Label name",
    labelPlaceholder: "Enter a label name…",
    addLabel: "Add label",
    addingLabel: "Adding label…",
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
    downloadError: "Attachment download failed; try again.",
    mutationError: "Operation failed",
    reloadStale: "The write committed, but refresh failed; data may be stale.",
    retry: "Retry original submission",
    retrying: "Retrying original submission…",
    error: "Operation failed. Try again.",
    clearFile: "Clear selected file",
    invalidFileType: "This file type is not supported.",
    fileTooLarge: (size) => `The file exceeds the ${size} upload limit.`,
    fileCountExceeded: "Choose one file at a time.",
  },
}

function normalizedLabelName(value: string): string {
  return value.trim().toLowerCase()
}

function errorMessage(error: unknown, fallback: string): string {
  if (error instanceof Error && error.message.trim()) return error.message
  if (typeof error === "string" && error.trim()) return error
  return fallback
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
  if (atoms.length === 0) return <Text type="supporting">—</Text>
  return (
    <SafeVStack gap={1}>
      {atoms.map((atom, index) => (
        <Text
          key={`${atom.polarity}-${atom.atom_id}-${index}`}
          type="supporting"
          wordBreak="break-word"
        >
          {atom.text}
        </Text>
      ))}
    </SafeVStack>
  )
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
    <ListItem
      label={<Text type="label" wordBreak="break-word">{entry.label_name}</Text>}
      description={(
        <SafeVStack gap={1}>
          <Text type="supporting">{copy.score(entry.score.toFixed(3))}</Text>
          <Evidence entry={entry} />
        </SafeVStack>
      )}
      endContent={(
        <Button
          type="button"
          size="sm"
          variant="secondary"
          label={`${applied ? copy.applied : copy.apply} ${entry.label_name}`}
          aria-label={`${applied ? copy.applied : copy.apply} ${entry.label_name}`}
          data-testid="label-suggestion-apply"
          isDisabled={disabled}
          onClick={() => onApply(entry.label_name)}
        />
      )}
    />
  )
}

function AttachmentMetadata({ attachment, copy, locale }: { readonly attachment: InspectorAssetAttachment; readonly copy: AssetsCopy; readonly locale: Locale }) {
  const createdAt = attachmentTime(attachment.created_at, locale)
  return (
    <SafeMetadataList columns="single" label={{ position: "start" }}>
      <SafeMetadataListItem label={copy.filename}>
        <Text type="code" wordBreak="break-word">{attachment.filename}</Text>
      </SafeMetadataListItem>
      <SafeMetadataListItem label={copy.contentType}>
        <Text type="code" wordBreak="break-word">{attachment.content_type ?? "—"}</Text>
      </SafeMetadataListItem>
      <SafeMetadataListItem label={copy.size}>{formatAttachmentSize(attachment.size_bytes)}</SafeMetadataListItem>
      <SafeMetadataListItem label={copy.sha256}>
        <Text type="code" wordBreak="break-word">{attachment.sha256 ?? "—"}</Text>
      </SafeMetadataListItem>
      <SafeMetadataListItem label={copy.createdBy}>
        <Text type="code" wordBreak="break-word">{attachment.created_by}</Text>
      </SafeMetadataListItem>
      <SafeMetadataListItem label={copy.createdAt}>
        <time dateTime={createdAt.iso}>{createdAt.display}</time>
      </SafeMetadataListItem>
    </SafeMetadataList>
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
  const mountedRef = useRef(false)
  const localBusyRef = useRef(new Set<string>())
  const [localBusy, setLocalBusy] = useState<ReadonlySet<string>>(new Set())
  const [localErrors, setLocalErrors] = useState<ReadonlyMap<string, string>>(new Map())
  const [labelInput, setLabelInput] = useState("")
  const [selectedFile, setSelectedFile] = useState<File | null>(null)
  const labelInputValueRef = useRef(labelInput)
  const labelInputElementRef = useRef<HTMLInputElement | null>(null)
  const selectedFileRef = useRef<File | null>(selectedFile)
  const fileInputElementRef = useRef<HTMLInputElement | null>(null)
  const attemptedUploadFileRef = useRef<File | null>(null)
  const [fileInputKey, setFileInputKey] = useState(0)
  const [suggestionBusy, setSuggestionBusy] = useState(false)
  const [suggestionLocalError, setSuggestionLocalError] = useState<string | null>(null)
  const [suggestionLocalRequested, setSuggestionLocalRequested] = useState(false)
  const [suggestionLocalResult, setSuggestionLocalResult] = useState<InspectorLabelSuggestionResult | null | undefined>(undefined)
  const retryBusyRef = useRef(new Set<string>())
  const [retryBusy, setRetryBusy] = useState<ReadonlySet<string>>(new Set())

  // scope、mounted state 和 draft ref 只在 layout effect 中随 commit 更新。
  // React 放弃的 render 不得使 live promise 失效或替换上一次提交的 draft。
  useLayoutEffect(() => {
    const localBusy = localBusyRef.current
    const retryBusy = retryBusyRef.current
    mountedRef.current = true
    return () => {
      mountedRef.current = false
      localBusy.clear()
      retryBusy.clear()
      attemptedUploadFileRef.current = null
    }
  }, [])

  useLayoutEffect(() => {
    const previous = scopeIdentityRef.current
    const next = advanceInspectorAssetsScope(previous, taskId, snapshot.generation)
    if (next === previous) return
    scopeIdentityRef.current = next
    localBusyRef.current.clear()
    setLocalBusy(new Set())
    setLocalErrors(new Map())
    setLabelInput("")
    setSelectedFile(null)
    attemptedUploadFileRef.current = null
    setFileInputKey((current) => current + 1)
    setSuggestionBusy(false)
    setSuggestionLocalError(null)
    setSuggestionLocalRequested(false)
    setSuggestionLocalResult(undefined)
    retryBusyRef.current.clear()
    setRetryBusy(new Set())
  }, [snapshot.generation, taskId])

  useLayoutEffect(() => {
    labelInputValueRef.current = labelInput
  }, [labelInput])

  useLayoutEffect(() => {
    selectedFileRef.current = selectedFile
  }, [selectedFile])

  const isCurrentScope = useCallback((captured: InspectorAssetsScopeIdentity): boolean => {
    return mountedRef.current && isInspectorAssetsScopeCurrent(scopeIdentityRef.current, captured)
  }, [])

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
      if (isCurrentScope(identity)) setLocalErrors((current) => new Map(current).set(key, errorMessage(error, copy.error)))
      return null
    } finally {
      if (isCurrentScope(identity)) {
        localBusyRef.current.delete(key)
        setLocalBusy(new Set(localBusyRef.current))
      }
    }
  }, [copy.error, isCurrentScope, pending])

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
    if (shouldClearAssetDraft(outcome) && isLabelRetryDraftCurrent(labelInputValueRef.current, name)) setLabelInput("")
  }, [actions, existingLabelNames, labelInput, runAction])
  const onLabelSubmit = useCallback((event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    void addLabel()
  }, [addLabel])
  const removeLabelAction = useCallback((labelId: string) => {
    void runAction("removeLabel", () => actions.removeLabel(labelId))
  }, [actions, runAction])

  const currentSuggestions = suggestionLocalResult !== undefined
    ? suggestionLocalResult?.task_id === taskId
      ? suggestionLocalResult
      : null
    : suggestionResult?.task_id === taskId
      ? suggestionResult
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
    setSuggestionLocalRequested(true)
    setSuggestionLocalError(null)
    setSuggestionBusy(true)
    void requestSuggestedLabels(handlers.suggestLabels)
      .then((response) => {
        if (!isCurrentScope(identity)) return
        setSuggestionLocalResult(response?.data ?? null)
      })
      .catch((error: unknown) => {
        if (isCurrentScope(identity)) setSuggestionLocalError(errorMessage(error, copy.error))
      })
      .finally(() => {
        if (isCurrentScope(identity)) setSuggestionBusy(false)
      })
  }, [copy.error, handlers, isCurrentScope, suggestionPending])

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
      if (!downloaded || typeof document === "undefined" || typeof URL === "undefined" || typeof URL.createObjectURL !== "function" || typeof URL.revokeObjectURL !== "function") {
        throw new Error(copy.downloadError)
      }
      const blob = new Blob([exactAttachmentBytes(downloaded.content)], { type: downloaded.content_type ?? attachment.content_type ?? "application/octet-stream" })
      const url = URL.createObjectURL(blob)
      const anchor = document.createElement("a")
      anchor.href = url
      anchor.download = attachment.filename
      anchor.click()
      anchor.remove()
      window.setTimeout(() => URL.revokeObjectURL(url), 0)
    })
  }, [actions, copy.downloadError, runAction])

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
        if (operation === "addLabel" && intent?.operation === "addLabel" && typeof intent.input.name === "string" && isLabelRetryDraftCurrent(labelInputValueRef.current, intent.input.name)) setLabelInput("")
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
  const addLabelError = errorFor(["addLabel"])
  const suggestSnapshotError = errors.get(actionKey("suggestLabels", taskId))?.message ?? null
  const suggestionsError = suggestionLocalError ?? suggestionError ?? errorFor(["suggestLabels"]) ?? suggestSnapshotError
  const attachmentsError = attachmentError ?? errorFor(["uploadAttachment", "downloadAttachment", "deleteAttachment"])
  const uploadError = errorFor(["uploadAttachment"])
  const addLabelSnapshotError = errors.get(actionKey("addLabel", taskId))?.message ?? null
  const uploadSnapshotError = errors.get(actionKey("uploadAttachment", taskId))?.message ?? null
  const addLabelFocusError = addLabelError ?? addLabelSnapshotError
  const uploadFocusError = uploadError ?? uploadSnapshotError ?? attachmentError
  const labelErrorId = `${panelId}-label-error`
  const attachmentErrorId = `${panelId}-attachment-error`
  const showSuggestions = suggestionRequested || suggestionLocalRequested || Boolean(currentSuggestions) || suggestionPending || Boolean(suggestionsError)
  const duplicateIdsText = [...duplicateSuggestionIds].join(", ")
  const uploadPending = isPending("uploadAttachment")
  const snapshotErrors = useMemo(
    () => [...errors.entries()].filter(([key, error]) => shouldShowInspectorSnapshotError(key, error, taskId, pending, retryBusy, localErrors)),
    [errors, localErrors, pending, retryBusy, taskId],
  )
  const canRetrySnapshotError = useCallback((key: string, operation: string): boolean => {
    if (!snapshot.retries.has(key) || operation === "suggestLabels" || operation === "downloadAttachment") return false
    return true
  }, [snapshot.retries])
  const addLabelRetryIntent = snapshot.retries.get(actionKey("addLabel", taskId))
  const addLabelRetryLocked = addLabelRetryIntent?.operation === "addLabel"
    && typeof addLabelRetryIntent.input.name === "string"
    && isLabelRetryDraftCurrent(labelInput, addLabelRetryIntent.input.name)
  const uploadRetryIntent = snapshot.retries.get(actionKey("uploadAttachment", taskId))
  const uploadRetryLocked = uploadRetryIntent?.operation === "uploadAttachment"
    && selectedFile !== null
    && isAttachmentRetryDraftCurrent(selectedFile, attemptedUploadFileRef.current)
  const panelBusy = suggestionPending || writePending || isPending("downloadAttachment") || attachmentLoading
  const retrying = retryBusy.size > 0 || isPending("reload")

  useLayoutEffect(() => {
    if (addLabelFocusError) labelInputElementRef.current?.focus()
  }, [addLabelFocusError])

  useLayoutEffect(() => {
    if (uploadFocusError) fileInputElementRef.current?.focus()
  }, [uploadFocusError])

  return (
    <SafeVStack
      as="section"
      gap={5}
      data-testid="inspector-assets"
      aria-labelledby={`${panelId}-heading`}
      aria-busy={panelBusy}
    >
      <SafeHStack as="header" justify="between" align="center" wrap="wrap" gap={2}>
        <Heading level={2} id={`${panelId}-heading`}>{copy.title}</Heading>
        <Text type="code" wordBreak="break-word">{taskId}</Text>
      </SafeHStack>

      {snapshotErrors.length > 0 ? (
        <SafeVStack gap={2} data-testid="inspector-mutation-errors">
          {snapshotErrors.map(([key, error]) => {
            const errorId = key === actionKey("addLabel", taskId)
              ? labelErrorId
              : key === actionKey("uploadAttachment", taskId)
                ? attachmentErrorId
                : undefined
            const retryable = canRetrySnapshotError(key, error.operation)
            return (
              <Banner
                key={key}
                id={errorId}
                data-testid="inspector-mutation-error"
                data-operation-key={key}
                status={error.operation === "reload" ? "warning" : "error"}
                role="alert"
                aria-live="polite"
                title={error.operation === "reload" ? copy.reloadStale : copy.mutationError}
                description={error.message}
                endContent={retryable ? (
                  <Button
                    type="button"
                    size="sm"
                    variant="secondary"
                    label={retryBusy.has(key) ? copy.retrying : copy.retry}
                    data-testid="inspector-retry"
                    data-retry-key={key}
                    isDisabled={retryBusy.has(key) || pending.has(key) || writePending}
                    isLoading={retryBusy.has(key)}
                    onClick={() => retryMutation(key)}
                  />
                ) : undefined}
              />
            )
          })}
        </SafeVStack>
      ) : null}
      {retrying ? (
        <Text as="p" type="supporting" data-testid="inspector-retry-status" role="status" aria-live="polite">
          {copy.retrying}
        </Text>
      ) : null}

      <SafeVStack as="section" gap={3} data-testid="inspector-labels" aria-labelledby={`${panelId}-labels-heading`}>
        <Heading level={3} id={`${panelId}-labels-heading`}>{copy.labels}</Heading>
        {labels.length > 0 ? (
          <List density="compact" hasDividers data-testid="inspector-label-list">
            {labels.map((label) => {
              const removeLabel = copy.removeLabel(label.name)
              return (
                <ListItem
                  key={label.id}
                  label={<Text wordBreak="break-word">{label.name}</Text>}
                  endContent={(
                    <Button
                      type="button"
                      size="sm"
                      variant="ghost"
                      label={removeLabel}
                      aria-label={removeLabel}
                      data-testid="label-remove"
                      isDisabled={writePending}
                      onClick={() => removeLabelAction(label.id)}
                    />
                  )}
                />
              )
            })}
          </List>
        ) : (
          <Text as="p" type="supporting" data-testid="labels-empty" role="status">{copy.noLabels}</Text>
        )}
        <form onSubmit={onLabelSubmit} aria-busy={writePending || undefined}>
          <SafeHStack align="end" gap={2} wrap="wrap">
            <TextInput
              id={labelInputId}
              label={copy.labelName}
              value={labelInput}
              onChange={(value) => setLabelInput(value)}
              placeholder={copy.labelPlaceholder}
              htmlName="label-name"
              autoComplete="off"
              isDisabled={writePending}
              status={addLabelFocusError ? { type: "error" } : undefined}
              aria-describedby={addLabelFocusError ? labelErrorId : undefined}
              ref={labelInputElementRef}
            />
            <Button
              type="submit"
              size="sm"
              variant="primary"
              label={isPending("addLabel") ? copy.addingLabel : copy.addLabel}
              data-testid="label-add"
              isDisabled={!labelInput.trim() || existingLabelNames.has(normalizedLabelName(labelInput)) || writePending || addLabelRetryLocked}
              isLoading={isPending("addLabel")}
            />
          </SafeHStack>
        </form>
        {labelsError ? <Banner id={labelErrorId} status="error" role="alert" title={copy.mutationError} description={labelsError} container="section" /> : null}
        <Button
          type="button"
          size="sm"
          variant="secondary"
          label={suggestionPending ? copy.suggesting : suggestionRequested || suggestionLocalRequested || currentSuggestions ? copy.refreshSuggestions : copy.suggestLabels}
          data-testid="label-suggestion-request"
          isDisabled={suggestionPending}
          isLoading={suggestionPending}
          onClick={requestSuggestions}
        />
        {showSuggestions ? (
          <SafeVStack as="section" gap={3} data-testid="label-suggestions">
            {currentSuggestions ? (
              <Text type="supporting">
                {copy.coverage(suggestionPercent(currentSuggestions.coverage), suggestionPercent(currentSuggestions.coverage_cosine), suggestionResidual(currentSuggestions.residual_norm))}
              </Text>
            ) : null}
            {suggestionPending && !currentSuggestions ? <Text as="p" type="supporting" role="status" aria-live="polite">{copy.suggesting}</Text> : null}
            {suggestionsError ? <Banner status="error" role="alert" title={copy.mutationError} description={suggestionsError} container="section" /> : null}
            {currentSuggestions ? (
              <SafeMetadataList data-testid="label-suggestion-provenance" columns="single" label={{ position: "start" }}>
                <SafeMetadataListItem label={copy.reasonCodes}>{currentSuggestions.reason_codes.length ? currentSuggestions.reason_codes.join(", ") : "—"}</SafeMetadataListItem>
                <SafeMetadataListItem label={copy.diagnostics}>{currentSuggestions.diagnostics.length ? currentSuggestions.diagnostics.join(", ") : "—"}</SafeMetadataListItem>
              </SafeMetadataList>
            ) : null}
            {currentSuggestions?.degraded ? (
              <Banner
                status="warning"
                role="alert"
                title={copy.degraded}
                description={copy.degradedDescription}
                container="section"
              />
            ) : null}
            {duplicateSuggestionIds.size > 0 ? (
              <Banner
                status="error"
                role="alert"
                data-testid="label-suggestion-duplicates"
                title={copy.duplicateSuggestion(duplicateIdsText)}
                container="section"
              />
            ) : null}
            {selectedSuggestions.length > 0 ? (
              <SafeVStack as="section" gap={2}>
                <Heading level={4}>{copy.selected}</Heading>
                <List density="compact" hasDividers>
                  {selectedSuggestions.map((entry) => (
                    <SuggestionRow
                      key={`selected-${entry.label_id}-${entry.label_name}`}
                      entry={entry}
                      copy={copy}
                      applied={entry.already_applied || existingLabelNames.has(normalizedLabelName(entry.label_name))}
                      disabled={writePending || entry.already_applied || duplicateSuggestionIds.has(entry.label_id) || existingLabelNames.has(normalizedLabelName(entry.label_name))}
                      onApply={applySuggestion}
                    />
                  ))}
                </List>
              </SafeVStack>
            ) : null}
            {candidateSuggestions.length > 0 ? (
              <SafeVStack as="section" gap={2}>
                <Heading level={4}>{copy.candidates}</Heading>
                <List density="compact" hasDividers>
                  {candidateSuggestions.map((entry) => (
                    <SuggestionRow
                      key={`candidate-${entry.label_id}-${entry.label_name}`}
                      entry={entry}
                      copy={copy}
                      applied={entry.already_applied || existingLabelNames.has(normalizedLabelName(entry.label_name))}
                      disabled={writePending || entry.already_applied || duplicateSuggestionIds.has(entry.label_id) || existingLabelNames.has(normalizedLabelName(entry.label_name))}
                      onApply={applySuggestion}
                    />
                  ))}
                </List>
              </SafeVStack>
            ) : null}
            {!suggestionPending && !suggestionsError && (suggestionRequested || suggestionLocalRequested) && (!currentSuggestions || (selectedSuggestions.length === 0 && candidateSuggestions.length === 0)) ? (
              <Text as="p" type="supporting" role="status">{copy.noSuggestions}</Text>
            ) : null}
          </SafeVStack>
        ) : null}
      </SafeVStack>

      <SafeVStack as="section" gap={3} data-testid="inspector-attachments" aria-labelledby={`${panelId}-attachments-heading`}>
        <Heading level={3} id={`${panelId}-attachments-heading`}>{copy.attachments}</Heading>
        <SafeHStack align="end" gap={2} wrap="wrap">
          <FileInput
            key={fileInputKey}
            id={attachmentFileId}
            htmlName="attachment-file"
            data-testid="attachment-file"
            label={copy.chooseFile}
            value={selectedFile}
            onChange={(files) => setSelectedFile(Array.isArray(files) ? files[0] ?? null : files)}
            maxSize={MAX_ATTACHMENT_UPLOAD_BYTES}
            isDisabled={writePending}
            isLoading={uploadPending}
            status={uploadFocusError ? { type: "error" } : undefined}
            aria-label={copy.chooseFile}
            aria-describedby={uploadFocusError ? attachmentErrorId : undefined}
            chooseFileText={copy.chooseFile}
            chooseFilesText={copy.chooseFile}
            clearLabel={copy.clearFile}
            clearText={copy.clearFile}
            invalidTypeMessage={copy.invalidFileType}
            sizeLimitMessage={(_file, _maxSize, formattedSize) => copy.fileTooLarge(formattedSize)}
            maxFilesMessage={copy.fileCountExceeded}
            formatFileSize={formatAttachmentSize}
            ref={fileInputElementRef}
          />
          <Button
            type="button"
            size="sm"
            variant="primary"
            label={uploadPending ? copy.uploading : copy.upload}
            data-testid="attachment-upload"
            isDisabled={!selectedFile || writePending || uploadRetryLocked}
            isLoading={uploadPending}
            onClick={uploadFile}
          />
        </SafeHStack>
        {attachmentLoading ? <Text as="p" type="supporting" data-testid="attachments-loading" role="status" aria-live="polite">{copy.loadingAttachments}</Text> : null}
        {attachmentsError ? <Banner id={attachmentErrorId} status="error" role="alert" data-testid="attachments-error" title={copy.attachmentError} description={attachmentsError} container="section" /> : null}
        {attachments.length > 0 ? (
          <List density="compact" hasDividers data-testid="attachment-list">
            {attachments.map((attachment) => {
              const downloadLabel = copy.download(attachment.filename)
              const deleteLabel = copy.deleteAttachment(attachment.filename)
              return (
                <ListItem
                  key={attachment.id}
                  data-testid="attachment-row"
                  label={<Text type="code" wordBreak="break-word">{attachment.filename}</Text>}
                  description={<AttachmentMetadata attachment={attachment} copy={copy} locale={locale} />}
                  endContent={(
                    <SafeHStack gap={1} wrap="wrap" justify="end">
                      <Button
                        type="button"
                        size="sm"
                        variant="secondary"
                        label={isPending("downloadAttachment") ? copy.downloading : downloadLabel}
                        aria-label={downloadLabel}
                        data-testid="attachment-download"
                        isDisabled={isPending("downloadAttachment")}
                        isLoading={isPending("downloadAttachment")}
                        onClick={() => downloadAttachment(attachment)}
                      />
                      <Button
                        type="button"
                        size="sm"
                        variant="destructive"
                        label={isPending("deleteAttachment") ? copy.deleting : deleteLabel}
                        aria-label={deleteLabel}
                        data-testid="attachment-delete"
                        isDisabled={writePending}
                        isLoading={isPending("deleteAttachment")}
                        onClick={() => deleteAttachment(attachment.id)}
                      />
                    </SafeHStack>
                  )}
                />
              )
            })}
          </List>
        ) : (
          <Text as="p" type="supporting" data-testid="attachments-empty" role="status">{copy.noAttachments}</Text>
        )}
      </SafeVStack>
    </SafeVStack>
  )
}
