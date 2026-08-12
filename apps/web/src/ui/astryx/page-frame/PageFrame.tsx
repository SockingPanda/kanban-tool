import type { ReactNode, Ref } from "react"

export type PageFrameMode = "content" | "workspace"
export type PageFrameBodyOverflow = "none" | "x" | "y" | "both"

/**
 * Literal classes from the Astryx Tailwind bridge.
 *
 * The frame owns only the page-level inset. A workspace body stays full-bleed
 * so its caller can put overflow on the narrow data region that owns it.
 */
export const PAGE_FRAME_CLASSES = {
  root: {
    content: "min-w-0 p-4",
    workspace: "min-w-0",
  },
  header: {
    content: "min-w-0",
    workspace: "min-w-0 p-4",
  },
  toolbar: {
    content: "min-w-0",
    workspace: "min-w-0 p-4",
  },
  body: {
    content: "min-w-0",
    workspace: "min-w-0",
  },
} as const satisfies Readonly<{
  readonly root: Readonly<Record<PageFrameMode, string>>
  readonly header: Readonly<Record<PageFrameMode, string>>
  readonly toolbar: Readonly<Record<PageFrameMode, string>>
  readonly body: Readonly<Record<PageFrameMode, string>>
}>

export const PAGE_FRAME_BODY_OVERFLOW_CLASSES = {
  none: "min-w-0",
  x: "min-w-0 overflow-x-auto",
  y: "min-w-0 overflow-y-auto",
  both: "min-w-0 overflow-auto",
} as const satisfies Readonly<Record<PageFrameBodyOverflow, string>>

type PageFrameRootProps = {
  readonly id?: string
  readonly "aria-label"?: string
  readonly "aria-labelledby"?: string
  readonly "data-testid"?: string
  readonly ref?: Ref<HTMLElement>
  readonly header?: ReactNode
  readonly children: ReactNode
  readonly bodyOverflow?: PageFrameBodyOverflow
}

type ToolbarSlot =
  | {
      readonly toolbar?: never
      readonly toolbarLabel?: never
    }
  | {
      readonly toolbar: ReactNode
      readonly toolbarLabel: string
    }

type OptionalBodyName =
  | {
      readonly bodyLabel?: string
      readonly bodyLabelledBy?: never
    }
  | {
      readonly bodyLabel?: never
      readonly bodyLabelledBy?: string
    }

type RequiredBodyName =
  | {
      readonly bodyLabel: string
      readonly bodyLabelledBy?: never
    }
  | {
      readonly bodyLabel?: never
      readonly bodyLabelledBy: string
    }

export type ContentPageFrameProps = PageFrameRootProps &
  ToolbarSlot &
  OptionalBodyName & {
    readonly frame: "content"
  }

export type WorkspacePageFrameProps = PageFrameRootProps &
  ToolbarSlot &
  RequiredBodyName & {
    readonly frame: "workspace"
  }

export type PageFrameProps = ContentPageFrameProps | WorkspacePageFrameProps

/**
 * Static, strict-CSP page frame for content and dense workspace surfaces.
 *
 * The root is intentionally a section rather than a main landmark: the
 * product shell owns the document's main landmark. Workspace callers must
 * name their body region and choose any local overflow owner explicitly.
 */
export function PageFrame(props: PageFrameProps) {
  const {
    "aria-label": ariaLabel,
    "aria-labelledby": ariaLabelledBy,
    bodyLabel,
    bodyLabelledBy,
    bodyOverflow = "none",
    children,
    ["data-testid"]: dataTestId,
    frame,
    header,
    id,
    ref,
    toolbar,
    toolbarLabel,
  } = props

  const hasHeader = header !== undefined && header !== null
  const hasToolbar = toolbar !== undefined && toolbar !== null
  const bodyClassName = frame === "workspace"
    ? PAGE_FRAME_BODY_OVERFLOW_CLASSES[bodyOverflow]
    : PAGE_FRAME_CLASSES.body.content

  return (
    <section
      ref={ref}
      id={id}
      aria-label={ariaLabel}
      aria-labelledby={ariaLabelledBy}
      className={PAGE_FRAME_CLASSES.root[frame]}
      data-frame={frame}
      data-testid={dataTestId}
    >
      {hasHeader ? <header className={PAGE_FRAME_CLASSES.header[frame]}>{header}</header> : null}
      {hasToolbar ? (
        <section
          aria-label={toolbarLabel}
          className={PAGE_FRAME_CLASSES.toolbar[frame]}
          role="toolbar"
        >
          {toolbar}
        </section>
      ) : null}
      {frame === "workspace" ? (
        <section
          aria-label={bodyLabel}
          aria-labelledby={bodyLabelledBy}
          className={bodyClassName}
          data-body-overflow={bodyOverflow}
          role="region"
        >
          {children}
        </section>
      ) : (
        <article
          aria-label={bodyLabel}
          aria-labelledby={bodyLabelledBy}
          className={bodyClassName}
        >
          {children}
        </article>
      )}
    </section>
  )
}

PageFrame.displayName = "CspSafePageFrame"
