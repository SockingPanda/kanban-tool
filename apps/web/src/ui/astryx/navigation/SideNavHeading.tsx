import { cloneElement, isValidElement, type ReactElement, type ReactNode } from "react"

import { joinClassNames } from "./classNames"
import { useSideNavContext } from "./context"
import type { SideNavHeadingProps } from "./types"

const headingClasses = {
  root: "flex min-h-12 items-center gap-2 border-b border-border px-3 py-2",
  rootCollapsed: "justify-center px-2",
  copy: "min-w-0 flex-1",
  superheading: "block truncate text-xs text-secondary",
  subheading: "block truncate text-xs text-secondary",
  link: "no-underline outline-none focus-visible:outline-2 focus-visible:outline-accent",
  collapsedCopy: "sr-only",
  endContent: "ms-auto shrink-0",
} as const

function HeadingLine({
  content,
  href,
  className,
}: {
  readonly content: ReactNode
  readonly href?: string
  readonly className: string
}) {
  if (href === undefined) return <strong className={className}>{content}</strong>
  return <a className={joinClassNames(className, headingClasses.link)} href={href}>{content}</a>
}

function iconNode(icon: ReactNode): ReactNode {
  return isValidElement(icon) ? cloneElement(icon as ReactElement<{ "aria-hidden"?: boolean }>, { "aria-hidden": true }) : icon
}

export function SideNavHeading({
  heading,
  icon,
  headingHref,
  superheading,
  superheadingHref,
  subheading,
  subheadingHref,
  headerEndContent,
  className,
  "data-testid": testId,
}: SideNavHeadingProps) {
  const { isCollapsed } = useSideNavContext()
  const copyClassName = joinClassNames(headingClasses.copy, isCollapsed && headingClasses.collapsedCopy)

  return (
    <header className={joinClassNames(headingClasses.root, isCollapsed && headingClasses.rootCollapsed, className)} data-collapsed={isCollapsed ? "true" : "false"}>
      {headingHref === undefined ? (
        <h2 className={joinClassNames("m-0 flex min-w-0 items-center gap-2", isCollapsed && "justify-center")} data-testid={testId}>
          {icon !== undefined ? iconNode(icon) : null}
          <strong className={copyClassName}>{heading}</strong>
        </h2>
      ) : (
        <h2 className={joinClassNames("m-0 flex min-w-0 items-center gap-2", isCollapsed && "justify-center")}>
          <a className={joinClassNames("flex min-w-0 items-center gap-2", headingClasses.link)} href={headingHref} data-testid={testId}>
            {icon !== undefined ? iconNode(icon) : null}
            <strong className={copyClassName}>{heading}</strong>
          </a>
        </h2>
      )}
      {!isCollapsed && superheading !== undefined ? (
        <p className={headingClasses.superheading}>
          <HeadingLine content={superheading} href={superheadingHref} className={headingClasses.superheading} />
        </p>
      ) : null}
      {!isCollapsed && subheading !== undefined ? (
        <p className={headingClasses.subheading}>
          <HeadingLine content={subheading} href={subheadingHref} className={headingClasses.subheading} />
        </p>
      ) : null}
      {!isCollapsed && headerEndContent !== undefined ? <section className={headingClasses.endContent} role="presentation">{headerEndContent}</section> : null}
    </header>
  )
}

export default SideNavHeading
