/* eslint-disable react-refresh/only-export-components */
import { useCallback, useState, type HTMLAttributes, type Ref } from "react"

import {
  guardNoRuntimeStyleProps,
  type NoRuntimeStyleProps,
} from "./safe-core"

export type CodeBlockHeight = "none" | "compact" | "evidence"
export type CodeBlockContainer = "card" | "section"

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
  extends Omit<HTMLAttributes<HTMLElement>, "children" | "style" | "title">,
    NoRuntimeStyleProps {
  readonly code: string
  readonly language?: string
  readonly isWrapped?: boolean
  readonly container?: CodeBlockContainer
  readonly maxHeight?: CodeBlockHeight
  readonly label?: string
  readonly hasCopy?: boolean
  readonly onCopy?: () => void
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
  onCopy,
  className,
  ref,
  ...rest
}: CodeBlockProps) {
  const [copied, setCopied] = useState(false)
  const copy = useCallback(async () => {
    if (typeof navigator === "undefined" || !navigator.clipboard) return
    try {
      await navigator.clipboard.writeText(code)
      setCopied(true)
      onCopy?.()
    } catch {
      setCopied(false)
    }
  }, [code, onCopy])

  const safeRest = guardNoRuntimeStyleProps(rest)
  const resolvedLabel = label ?? `${language} code`

  return (
    <section
      ref={ref}
      aria-label={resolvedLabel}
      className={classNames(
        "relative min-w-0 overflow-hidden",
        CODE_BLOCK_CONTAINER_CLASSES[container],
        className,
      )}
      {...safeRest}
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
          aria-label={copied ? "Copied" : "Copy code"}
          onClick={() => void copy()}
        >
          {copied ? "Copied" : "Copy"}
        </button>
      ) : null}
      <p className="sr-only" aria-live="polite" aria-atomic="true" data-copy-status>
        {copied ? "Copied" : ""}
      </p>
    </section>
  )
}

CodeBlock.displayName = "CspSafeCodeBlock"
