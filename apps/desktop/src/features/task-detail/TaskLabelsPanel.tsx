import { Loader2, Plus, Sparkles, X } from "lucide-react"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Field, FieldLabel } from "@/components/ui/field"
import { InputGroup, InputGroupButton, InputGroupInput } from "@/components/ui/input-group"
import type { KanbanApi, LabelSuggestionResult, Task } from "@/lib/api"

export function TaskLabelsPanel({
  api,
  task,
  labelInput,
  setLabelInput,
  suggestions,
  suggestionsRequested,
  suggestionsLoading,
  suggestionsError,
  pending,
  onAddLabel,
  onRemoveLabel,
  onRequestLabelSuggestions,
  onApplySuggestedLabel,
}: {
  api: KanbanApi | null
  task: Task
  labelInput: string
  setLabelInput: (value: string) => void
  suggestions: LabelSuggestionResult | null
  suggestionsRequested: boolean
  suggestionsLoading: boolean
  suggestionsError: string | null
  pending: boolean
  onAddLabel: () => void
  onRemoveLabel: (labelId: string) => void
  onRequestLabelSuggestions?: () => void
  onApplySuggestedLabel: (labelName: string) => void
}) {
  return (
    <div className="min-w-0 space-y-3">
      <div className="flex min-w-0 max-w-full flex-wrap gap-1.5">
        {task.labels.length ? (
          task.labels.map((label) => (
            <span key={label.id} className="inline-flex max-w-full items-center overflow-hidden rounded-md border border-border bg-muted">
              <Badge variant="secondary" className="max-w-48 truncate rounded-r-none px-2">
                {label.name}
              </Badge>
              <Button
                type="button"
                variant="ghost"
                className="h-6 rounded-none px-1.5 text-muted-foreground hover:text-destructive"
                disabled={!api || pending}
                aria-label={`Remove label ${label.name}`}
                onClick={() => onRemoveLabel(label.id)}
              >
                <X className="h-3.5 w-3.5" />
              </Button>
            </span>
          ))
        ) : (
          <span className="text-sm text-muted-foreground">none</span>
        )}
      </div>
      <LabelSuggestionsPanel
        suggestions={suggestions}
        requested={suggestionsRequested}
        loading={suggestionsLoading}
        error={suggestionsError}
        pending={pending}
        disabled={!api}
        onRequest={onRequestLabelSuggestions}
        onApply={onApplySuggestedLabel}
      />
      <Field>
        <FieldLabel>Label name</FieldLabel>
        <InputGroup>
          <InputGroupInput aria-label="Label name" name="label-name" autoComplete="off" value={labelInput} onChange={(event) => setLabelInput(event.target.value)} placeholder="Label name" />
          <InputGroupButton variant="outline" aria-label="Add label" disabled={!api || !labelInput.trim() || pending} onClick={onAddLabel}>
            <Plus className="h-4 w-4" />
          </InputGroupButton>
        </InputGroup>
      </Field>
    </div>
  )
}

export async function applySuggestedTaskLabel(
  api: Pick<KanbanApi, "addTaskLabel"> | null,
  taskId: string,
  labelName: string,
  onAction: (action: () => Promise<unknown>, options?: { label?: string; fallbackTaskId?: string | null; invalidate?: "task" }) => Promise<unknown>,
) {
  if (!api) return undefined
  return onAction(() => api.addTaskLabel(taskId, labelName), { fallbackTaskId: taskId, label: "label", invalidate: "task" })
}

function labelSuggestionReasonLabel(code: string) {
  switch (code) {
    case "coverage_below_threshold":
      return "coverage gap"
    case "degraded_result":
      return "degraded result"
    case "label_atom_index_dirty":
      return "index dirty"
    case "label_atom_index_empty":
      return "index empty"
    case "label_atom_index_error":
      return "index error"
    case "no_selected_labels":
      return "no selected labels"
    case "residual_above_threshold":
      return "unexplained residual"
    case "vector_query_error":
      return "vector query error"
    case "vector_store_disabled":
      return "vector store disabled"
    default:
      return code.replace(/_/g, " ")
  }
}

function labelSuggestionReasonText(reasonCodes: string[]) {
  if (!reasonCodes.length) return "review required"
  return reasonCodes.map(labelSuggestionReasonLabel).join(", ")
}

function LabelSuggestionsPanel({
  suggestions,
  requested,
  loading,
  error,
  pending,
  disabled,
  onRequest,
  onApply,
}: {
  suggestions: LabelSuggestionResult | null
  requested: boolean
  loading: boolean
  error: string | null
  pending: boolean
  disabled: boolean
  onRequest?: () => void
  onApply: (labelName: string) => void
}) {
  const requestDisabled = disabled || loading || !onRequest
  const reasonText = suggestions ? labelSuggestionReasonText(suggestions.reason_codes) : null
  const requestButton = (
    <Button type="button" variant="outline" size="sm" disabled={requestDisabled} aria-label="Suggest labels" onClick={onRequest}>
      {loading ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Sparkles className="h-3.5 w-3.5" />}
      {loading ? "Suggesting…" : requested || suggestions ? "Refresh suggestions" : "Suggest labels"}
    </Button>
  )

  if (!requested && !suggestions && !loading && !error) {
    return requestButton
  }

  const visible = suggestions ? (suggestions.selected_labels.length ? suggestions.selected_labels : suggestions.candidates) : []
  return (
    <div className="min-w-0 w-full max-w-full space-y-2 overflow-hidden rounded-md border border-border p-2">
      <div className="flex min-w-0 flex-wrap items-center justify-between gap-2 text-xs">
        <span className="min-w-0 font-medium text-muted-foreground">Suggestions</span>
        <div className="flex min-w-0 flex-1 flex-wrap items-center justify-end gap-2">
          {suggestions ? (
            <span className="min-w-0 max-w-full break-words text-right text-muted-foreground">
              coverage {(suggestions.coverage * 100).toFixed(0)}% / cosine {(suggestions.coverage_cosine * 100).toFixed(0)}% / residual{" "}
              {suggestions.residual_norm.toFixed(3)}
            </span>
          ) : null}
          {requestButton}
        </div>
      </div>
      {loading && !suggestions ? <div className="text-xs text-muted-foreground">Finding label suggestions…</div> : null}
      {error ? (
        <Alert className="border-destructive/50 bg-destructive/5 py-2">
          <AlertTitle className="text-xs text-destructive">Suggestions failed</AlertTitle>
          <AlertDescription className="break-words text-xs text-destructive">{error}</AlertDescription>
        </Alert>
      ) : null}
      {suggestions?.needs_new_label ? (
        <div className="max-w-full rounded-sm border border-amber-300 bg-amber-50 px-2 py-1 text-xs text-amber-800">
          Existing label coverage needs review: {reasonText}
        </div>
      ) : null}
      {suggestions?.degraded ? (
        <Alert className="py-2">
          <AlertTitle className="text-xs">Degraded</AlertTitle>
          <AlertDescription className="break-words text-xs">{suggestions.diagnostics.join(", ")}</AlertDescription>
        </Alert>
      ) : null}
      {visible.length ? (
        <div className="min-w-0 max-w-full space-y-1.5">
          {visible.slice(0, 5).map((suggestion) => (
            <div key={suggestion.label_id} className="flex min-w-0 max-w-full items-start justify-between gap-2">
              <div className="min-w-0 flex-1 space-y-1">
                <div className="min-w-0 max-w-full truncate text-sm font-medium">{suggestion.label_name}</div>
                <div className="text-xs text-muted-foreground">score {suggestion.score.toFixed(3)}</div>
                {suggestion.evidence_atoms.length ? (
                  <div className="min-w-0 max-w-full space-y-0.5">
                    {suggestion.evidence_atoms.slice(0, 2).map((atom) => (
                      <div key={atom.atom_id} className="min-w-0 max-w-full truncate text-xs text-muted-foreground">
                        {atom.text}
                      </div>
                    ))}
                  </div>
                ) : null}
                {suggestion.negative_evidence_atoms.length ? <div className="text-xs text-muted-foreground">negative evidence {suggestion.negative_evidence_atoms.length}</div> : null}
              </div>
              <Button type="button" variant="outline" size="sm" disabled={disabled || pending || suggestion.already_applied} className="shrink-0" onClick={() => onApply(suggestion.label_name)}>
                <Plus className="h-3.5 w-3.5" />
                {suggestion.already_applied ? "Applied" : "Apply"}
              </Button>
            </div>
          ))}
        </div>
      ) : !loading && !error ? (
        <div className="text-xs text-muted-foreground">No label suggestions.</div>
      ) : null}
    </div>
  )
}
