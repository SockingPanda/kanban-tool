import { useId } from "react"

import { joinClassNames } from "./classNames"
import { useSideNavContext } from "./context"
import type { SideNavSectionProps } from "./types"

const sectionClasses = {
  item: "list-none",
  section: "py-2",
  headingRow: "flex items-center gap-2 px-3 pb-1",
  heading: "m-0 truncate text-xs font-semibold uppercase tracking-wide text-secondary",
  subtitle: "m-0 truncate px-3 text-xs text-secondary",
  endContent: "ml-auto shrink-0",
  list: "m-0 flex list-none flex-col gap-1 p-0",
  hiddenHeading: "sr-only",
} as const

export function SideNavSection({
  title,
  heading,
  subtitle,
  children,
  endContent,
  isHeaderHidden = false,
  className,
  "data-testid": testId,
}: SideNavSectionProps) {
  const { isCollapsed } = useSideNavContext()
  const headingId = useId()
  const resolvedHeading = heading ?? title ?? ""

  return (
    <li className={joinClassNames(sectionClasses.item, className)} role="presentation" data-testid={testId}>
      <section className={sectionClasses.section} aria-labelledby={headingId} data-collapsed={isCollapsed ? "true" : "false"}>
        <header className={joinClassNames(sectionClasses.headingRow, (isHeaderHidden || isCollapsed) && sectionClasses.hiddenHeading)}>
          <h2 id={headingId} className={sectionClasses.heading}>{resolvedHeading}</h2>
          {!isCollapsed && endContent !== undefined ? <aside className={sectionClasses.endContent}>{endContent}</aside> : null}
        </header>
        {!isCollapsed && subtitle !== undefined ? <p className={sectionClasses.subtitle}>{subtitle}</p> : null}
        <ul className={sectionClasses.list}>{children}</ul>
      </section>
    </li>
  )
}

export default SideNavSection
