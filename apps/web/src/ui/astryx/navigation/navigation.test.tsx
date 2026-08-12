import { readFileSync } from "node:fs"
import { resolve } from "node:path"

import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test } from "vitest"

import { SideNav } from "./SideNav"
import { SideNavHeading } from "./SideNavHeading"
import { SideNavItem } from "./SideNavItem"
import { SideNavSection } from "./SideNavSection"
import { TREE_GUIDE_CLASSES, TREE_LEVEL_CLASSES } from "./constants"
import { TreeList } from "./TreeList"

const navigationSource = (): string => {
  const sourceFiles = ["SideNav.tsx", "SideNavHeading.tsx", "SideNavItem.tsx", "SideNavSection.tsx", "TreeList.tsx"]
  return sourceFiles.map((fileName) => readFileSync(resolve(import.meta.dirname, fileName), "utf8")).join("\n")
}

const inlineStyleAttribute = ["style", "="].join("")

// Safe navigation deliberately does not expose runtime presentation escape hatches.
// @ts-expect-error style is intentionally not part of the safe navigation API.
const navigationStyle = <SideNav style={{}} />
// @ts-expect-error xstyle is intentionally not part of the safe navigation API.
const navigationXstyle = <SideNav xstyle={{}} />
// @ts-expect-error resize is intentionally not part of the safe navigation API.
const navigationResize = <SideNav resizable />
// @ts-expect-error a tree must be named by aria-label or header.
const unnamedTree = <TreeList items={[]} expandLabel={() => "展开"} collapseLabel={() => "收起"} />

void navigationStyle
void navigationXstyle
void navigationResize
void unnamedTree

const treeLabels = {
  expandLabel: (item: { id: string }) => `展开 ${item.id}`,
  collapseLabel: (item: { id: string }) => `收起 ${item.id}`,
}

describe("CSP-safe Astryx navigation primitives", () => {
  test("keeps SideNav semantic and does not expose resize, tooltip, or popover surfaces", () => {
    const markup = renderToStaticMarkup(
      <SideNav
        aria-label="Product navigation"
        collapsible={{ defaultIsCollapsed: true, hasButton: false }}
        data-testid="side-nav"
        header={<SideNavHeading heading="kanban-tool" headingHref="/app/" data-testid="side-nav-heading" />}
      >
        <SideNavSection heading="Workspace">
          <SideNavItem label="Projects" href="/app/" data-testid="side-nav-projects" />
        </SideNavSection>
      </SideNav>,
    )

    expect(markup).toContain("<nav")
    expect(markup).toContain('aria-label="Product navigation"')
    expect(markup).toContain('data-collapsed="true"')
    expect(markup).toContain('href="/app/"')
    expect(markup).toContain('data-testid="side-nav-heading"')
    expect(markup).toContain('data-testid="side-nav-projects"')
    expect(markup).not.toMatch(/tooltip|popover|resize/i)
    expect(markup).not.toContain(inlineStyleAttribute)
  })

  test("keeps item toggle labels caller-owned", () => {
    const markup = renderToStaticMarkup(
      <SideNavItem
        label="Projects"
        collapsible
        expandLabel="展开项目"
        collapseLabel="收起项目"
      >
        <SideNavItem label="Kanban" href="/app/boards/kanban" />
      </SideNavItem>,
    )

    expect(markup).toContain('aria-label="收起项目"')
    expect(markup).not.toMatch(/aria-label="(?:Expand|Collapse)/)
  })

  test("puts href, target, and testid on the real TreeList anchor", () => {
    const markup = renderToStaticMarkup(
      <TreeList
        aria-label="Projects"
        data-testid="project-tree"
        {...treeLabels}
        items={[{
          id: "projects",
          label: "Projects",
          isExpanded: true,
          children: [{
            id: "kanban",
            label: "Kanban",
            href: "/app/boards/kanban",
            target: "_self",
            testId: "tree-kanban",
            isSelected: true,
          }],
        }]}
      />,
    )

    expect(markup).toContain('<ul')
    expect(markup).toContain('role="tree"')
    expect(markup).toContain('role="treeitem"')
    expect(markup).toContain('aria-level="1"')
    expect(markup).toContain('aria-level="2"')
    expect(markup).toContain('aria-selected="true"')
    expect(markup).toContain('<a')
    expect(markup).toContain('href="/app/boards/kanban"')
    expect(markup).toContain('target="_self"')
    expect(markup).toContain('data-testid="tree-kanban"')
    expect(markup).toContain('aria-label="收起 projects"')
    expect(markup).not.toContain('data-testid="projects"')
    expect(markup).not.toContain(inlineStyleAttribute)
  })

  test("applies density to each tree row and keeps line guides explicit", () => {
    const item = { id: "root", label: "Root", isExpanded: true, children: [{ id: "child", label: "Child" }] }
    const compact = renderToStaticMarkup(<TreeList density="compact" items={[item]} aria-label="树" {...treeLabels} />)
    const spacious = renderToStaticMarkup(<TreeList density="spacious" items={[item]} aria-label="树" {...treeLabels} />)
    const noGuides = renderToStaticMarkup(<TreeList variant="noGuides" items={[item]} aria-label="树" {...treeLabels} />)

    expect(compact).toContain("py-1")
    expect(compact).not.toContain("py-3")
    expect(spacious).toContain("py-3")
    expect(spacious).not.toContain("py-1")
    expect(compact).toContain("border-l")
    expect(noGuides).not.toContain("border-l")
    expect(TREE_GUIDE_CLASSES[1]).toContain("border-l border-border")
    expect(TREE_GUIDE_CLASSES[6]).toContain("border-l border-border")
  })

  test("uses a bounded literal level map and one owned tree focus handler", () => {
    expect(Object.keys(TREE_LEVEL_CLASSES)).toEqual(["1", "2", "3", "4", "5", "6"])
    expect(Object.values(TREE_LEVEL_CLASSES).every((className) => /^pl-[0-9]+$/.test(className))).toBe(true)

    const source = navigationSource()
    expect(source).toContain('useTreeFocus<HTMLUListElement>')
    expect(source).toContain("hasRovingTabIndex: true")
    expect(source).toContain("onKeyDown={handleKeyDown}")
    expect(source).toContain("onFocus={handleTreeFocus}")
    expect(source).toContain("handleFocus(event)")
    expect(source).toContain("onToggleExpand")
    expect(source).toContain("onActivate")
    expect(source).not.toContain("onKeyDownCapture")
    expect(source).not.toMatch(/document\.(addEventListener|removeEventListener)/)
    expect(source).not.toMatch(/<(div|span)(\s|>)/)
    expect(source).not.toContain(inlineStyleAttribute)
    expect(source).not.toMatch(/<(style)(\s|>)/i)
    const forbiddenOverlayWords = ["Tool", "tip", "Pop", "over", "Resize", "able"]
    expect(source).not.toContain(forbiddenOverlayWords[0] + forbiddenOverlayWords[1])
    expect(source).not.toContain(forbiddenOverlayWords[2] + forbiddenOverlayWords[3])
    expect(source).not.toContain(forbiddenOverlayWords[4] + forbiddenOverlayWords[5])
    expect(source).not.toContain((forbiddenOverlayWords[4] + forbiddenOverlayWords[5]).toLowerCase())
    expect(source).not.toContain("Collapse navigation")
    expect(source).not.toMatch(/`(?:Collapse|Expand)\s/)
    expect(source).not.toMatch(/(?:collapseLabel|expandLabel)\s*\?\?\s*label/)
    expect(source).not.toMatch(/className=.*\$\{/)
    expect(source).not.toMatch(/(?:bg|text|border)-(?:neutral|sky|white)(?:-|\b)/)
    expect(source).not.toMatch(/(?:bg|text|border|ring|outline)-\[[^\]]+\]/)
    expect(source).toContain("text-primary")
    expect(source).toContain("text-secondary")
    expect(source).toContain("bg-surface")
    expect(source).toContain("border-border")
    expect(source).toContain("outline-accent")
  })
})
