/* eslint-disable react-refresh/only-export-components */
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type HTMLAttributes,
  type Ref,
} from "react"

import type {NoRuntimeStyleProps} from "./safe-core"
import {pickCspSafeDomProps} from "../dom-props"

export type CodeBlockHeight = "none" | "compact" | "evidence"
export type CodeBlockContainer = "card" | "section"
export type CodeBlockCopyState = "idle" | "copied" | "error"

/** Caller-provided copy feedback remains visible for a short, finite window. */
export const CODE_BLOCK_COPY_FEEDBACK_MS = 2_000

/**
 * Schedule the success-state reset without involving CSS or runtime styles.
 * Kept as a tiny export so the timer contract can be verified with fake timers.
 */
export function scheduleCopyFeedbackReset(
  setStatus: (status: CodeBlockCopyState) => void,
): () => void {
  const timerId = setTimeout(() => setStatus("idle"), CODE_BLOCK_COPY_FEEDBACK_MS)
  return () => clearTimeout(timerId)
}

export const CODE_BLOCK_HEIGHT_CLASSES: Readonly<Record<CodeBlockHeight, string>> = {
  none: "max-h-none",
  compact: "max-h-64",
  evidence: "max-h-96",
}

export const CODE_BLOCK_CONTAINER_CLASSES: Readonly<Record<CodeBlockContainer, string>> = {
  card: "rounded-md border border-border bg-muted text-primary",
  section: "rounded-none border-0 bg-transparent text-primary",
}

export const CODE_BLOCK_SIZE_CLASSES = {
  compact: "px-3 py-2 text-xs leading-5",
  evidence: "px-4 py-3 text-sm leading-6",
} as const

export interface CodeBlockProps
  extends Omit<
      HTMLAttributes<HTMLElement>,
      "children" | "style" | "title" | "aria-label"
    >,
    NoRuntimeStyleProps {
  readonly code: string
  readonly language?: string
  readonly isWrapped?: boolean
  readonly container?: CodeBlockContainer
  readonly maxHeight?: CodeBlockHeight
  readonly label: string
  readonly hasCopy?: boolean
  readonly copyLabel: string
  readonly copiedLabel: string
  readonly errorLabel: string
  readonly onCopy?: () => void
  readonly onCopyError?: (error: unknown) => void
  readonly "data-testid"?: string
  readonly ref?: Ref<HTMLElement>
}

function classNames(...values: Array<string | false | null | undefined>): string {
  return values.filter(Boolean).join(" ")
}

/**
 * Strict-CSP code evidence surface.
 *
 * This deliberately renders plain text in native `pre > code`. It has no
 * tokenizer, CSS custom highlight API, syntax-theme provider, inline style,
 * or runtime `<style>` insertion. Callers can still identify the source via
 * the language data attribute and accessible label.
 */
export function CodeBlock({
  code,
  language = "plaintext",
  isWrapped = false,
  container = "card",
  maxHeight = "none",
  label,
  hasCopy = true,
  copyLabel,
  copiedLabel,
  errorLabel,
  id,
  onCopy,
  onCopyError,
  className,
  ref,
  ...rest
}: CodeBlockProps) {
  const [copyState, setCopyState] = useState<CodeBlockCopyState>("idle")
  const resetCopyFeedback = useRef<(() => void) | null>(null)

  const clearCopyReset = useCallback(() => {
    resetCopyFeedback.current?.()
    resetCopyFeedback.current = null
  }, [])

  useEffect(() => clearCopyReset, [clearCopyReset])

  const markCopied = useCallback(() => {
    clearCopyReset()
    setCopyState("copied")
    resetCopyFeedback.current = scheduleCopyFeedbackReset((status) => {
      resetCopyFeedback.current = null
      setCopyState(status)
    })
  }, [clearCopyReset])

  const markCopyError = useCallback((error: unknown) => {
    clearCopyReset()
    setCopyState("error")
    onCopyError?.(error)
  }, [clearCopyReset, onCopyError])

  const copy = useCallback(async () => {
    if (typeof navigator === "undefined" || !navigator.clipboard) {
      markCopyError(new Error("Clipboard API unavailable"))
      return
    }
    try {
      await navigator.clipboard.writeText(code)
    } catch (error) {
      markCopyError(error)
      return
    }
    markCopied()
    onCopy?.()
  }, [code, markCopied, markCopyError, onCopy])

  const safeRest = pickCspSafeDomProps(rest)
  const resolvedLabel = label
  const currentCopyLabel = copyState === "copied"
    ? copiedLabel
    : copyState === "error"
      ? errorLabel
      : copyLabel
  const liveCopyLabel = copyState === "idle" ? "" : currentCopyLabel

  return (
    <section
      ref={ref}
      id={id}
      className={classNames(
        "relative min-w-0 overflow-hidden",
        CODE_BLOCK_CONTAINER_CLASSES[container],
        className,
      )}
      {...safeRest}
      aria-label={resolvedLabel}
      data-language={language}
      data-container={container}
      data-max-height={maxHeight}
    >
      <pre
        className={classNames(
          "m-0 overflow-auto font-mono",
          CODE_BLOCK_HEIGHT_CLASSES[maxHeight],
          isWrapped ? "whitespace-pre-wrap break-words" : "whitespace-pre",
          CODE_BLOCK_SIZE_CLASSES[container === "card" ? "evidence" : "compact"],
        )}
        tabIndex={0}
        aria-label={resolvedLabel}
      >
        <code data-language={language}>{code}</code>
      </pre>
      {hasCopy ? (
        <button
          type="button"
          className="absolute end-2 top-2 rounded-sm border border-border bg-surface px-2 py-1 text-xs text-primary"
          aria-label={currentCopyLabel}
          onClick={() => void copy()}
        >
          {currentCopyLabel}
        </button>
      ) : null}
      <p className="sr-only" aria-live="polite" aria-atomic="true" data-copy-status>
        {liveCopyLabel}
      </p>
    </section>
  )
}

CodeBlock.displayName = "CspSafeCodeBlock"
