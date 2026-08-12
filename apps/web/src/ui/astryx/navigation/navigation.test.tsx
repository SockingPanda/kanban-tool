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
import { isPrimaryNavigationClick } from "./interaction"
import type { TreeListItemData, TreeListProps } from "./types"

const navigationSource = (): string => {
  const sourceFiles = ["SideNav.tsx", "SideNavHeading.tsx", "SideNavItem.tsx", "SideNavSection.tsx", "TreeList.tsx", "constants.ts", "interaction.ts"]
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
const unnamedTreeProps: TreeListProps<TreeListItemData> = {
  items: [{ id: "root", label: "Root" }],
  expandLabel: () => "展开",
  collapseLabel: () => "收起",
}
// @ts-expect-error boolean collapsible would hide the caller-owned labels.
const booleanSideNav = <SideNav collapsible />
// @ts-expect-error child items require caller-owned expand/collapse labels.
const unlabeledSideNavItem = <SideNavItem label="Projects"><SideNavItem label="Kanban" href="/app/boards/kanban" /></SideNavItem>
type DomainTreeItem = { readonly key: string; readonly title: string }
const adaptedDomainTree = <TreeList<DomainTreeItem>
  aria-label="域树"
  items={[{ key: "root", title: "Root" }]}
  adapter={(item) => ({ id: item.key, label: item.title })}
  expandLabel={() => "展开"}
  collapseLabel={() => "收起"}
/>
const labelledTree = <TreeList
  aria-labelledby="projects-heading"
  items={[{ id: "root", label: "Root" }]}
  expandLabel={() => "展开"}
  collapseLabel={() => "收起"}
/>
// @ts-expect-error non-Astryx item shapes require an adapter.
const unsafeDomainTree = <TreeList<DomainTreeItem>
  aria-label="域树"
  items={[{ key: "root", title: "Root" }]}
  expandLabel={() => "展开"}
  collapseLabel={() => "收起"}
/>

void navigationStyle
void navigationXstyle
void navigationResize
void unnamedTreeProps
void booleanSideNav
void unlabeledSideNavItem
void adaptedDomainTree
void labelledTree
void unsafeDomainTree

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
    expect(markup).not.toContain('role="menu"')
    expect(markup).not.toContain("<aside")
  })

  test("requires caller-owned SideNav collapse labels and keeps collapsed copy accessible", () => {
    const markup = renderToStaticMarkup(
      <SideNav
        aria-label="产品导航"
        collapsible={{ defaultIsCollapsed: true, expandLabel: "展开导航", collapseLabel: "收起导航" }}
      />,
    )

    expect(markup).toContain('aria-label="展开导航"')
    expect(markup).toContain('aria-expanded="false"')
    expect(markup).toContain('class="sr-only"')
    expect(markup).toContain("展开导航")
    expect(markup).not.toMatch(/Collapse navigation|Expand navigation/i)
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

  test("only points aria-controls at a rendered SideNavItem region", () => {
    const closed = renderToStaticMarkup(
      <SideNavItem
        label="Projects"
        collapsible
        defaultIsExpanded={false}
        expandLabel="展开项目"
        collapseLabel="收起项目"
        aria-controls="projects-children"
      >
        <SideNavItem label="Kanban" href="/app/boards/kanban" />
      </SideNavItem>,
    )
    const parentCollapsed = renderToStaticMarkup(
      <SideNav collapsible={{ defaultIsCollapsed: true, hasButton: false }}>
        <SideNavItem
          label="Projects"
          expandLabel="展开项目"
          collapseLabel="收起项目"
          aria-controls="projects-children"
        >
          <SideNavItem label="Kanban" href="/app/boards/kanban" />
        </SideNavItem>
      </SideNav>,
    )

    expect(closed).not.toContain('aria-controls="projects-children"')
    expect(closed).not.toContain('id="projects-children"')
    expect(parentCollapsed).not.toContain('aria-controls="projects-children"')
    expect(parentCollapsed).not.toContain('id="projects-children"')
  })

  test("keeps a primary item action and its expansion toggle independently reachable", () => {
    const markup = renderToStaticMarkup(
      <SideNavItem
        label="Projects"
        href="/app/projects"
        aria-controls="projects-children"
        expandLabel="展开项目"
        collapseLabel="收起项目"
      >
        <SideNavItem label="Kanban" href="/app/boards/kanban" />
      </SideNavItem>,
    )

    expect(markup).toContain('href="/app/projects"')
    expect(markup).toContain('data-side-nav-primary="true"')
    expect(markup).toContain('aria-controls="projects-children"')
    expect(markup).toContain('aria-label="收起项目"')
    expect(markup).toContain('tabindex="0"')
    expect(markup).toContain('id="projects-children"')
    expect(markup).not.toContain('role="menu"')
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
    expect(markup).not.toContain('role="menu"')
    expect(markup).not.toContain("<aside")
    expect(markup).not.toContain(inlineStyleAttribute)
  })

  test("accepts an aria-labelledby-only tree name", () => {
    const markup = renderToStaticMarkup(
      <TreeList
        aria-labelledby="projects-heading"
        items={[{ id: "projects", label: "Projects" }]}
        {...treeLabels}
      />,
    )

    expect(markup).toContain('role="tree"')
    expect(markup).toContain('aria-labelledby="projects-heading"')
    expect(markup).not.toContain('aria-label=""')
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
    expect(compact).toContain("border-s")
    expect(noGuides).not.toContain("border-s")
    expect(TREE_GUIDE_CLASSES[1]).toContain("border-s border-border")
    expect(TREE_GUIDE_CLASSES[6]).toContain("border-s border-border")
    expect(Object.values(TREE_LEVEL_CLASSES).every((className) => /^ps-[0-9]+$/.test(className))).toBe(true)
  })

  test("separates controlled and uncontrolled expansion without a local prop override", () => {
    const item = { id: "root", label: "Root", isExpanded: true, children: [{ id: "child", label: "Child" }] }
    const controlledClosed = renderToStaticMarkup(
      <TreeList expandedIds={[]} items={[item]} aria-label="树" {...treeLabels} />,
    )
    const defaultOpen = renderToStaticMarkup(
      <TreeList defaultExpandedIds={["root"]} items={[{ ...item, isExpanded: false }]} aria-label="树" {...treeLabels} />,
    )

    expect(controlledClosed).toContain('aria-expanded="false"')
    expect(controlledClosed).not.toContain('aria-level="2"')
    expect(defaultOpen).toContain('aria-expanded="true"')
    expect(defaultOpen).toContain('aria-level="2"')
  })

  test("keeps actual aria depth while clamping only finite visual utilities", () => {
    const deepItems: TreeListItemData[] = Array.from({ length: 8 }, (_, index) => ({
      id: `level-${index + 1}`,
      label: `Level ${index + 1}`,
      isExpanded: index < 7,
      children: index < 7 ? [] : undefined,
    }))
    for (let index = deepItems.length - 2; index >= 0; index -= 1) {
      deepItems[index] = { ...deepItems[index], children: [deepItems[index + 1]] }
    }
    const markup = renderToStaticMarkup(<TreeList items={[deepItems[0]]} aria-label="深树" {...treeLabels} />)

    expect(markup).toContain('aria-level="8"')
    expect(markup).toContain("ps-12")
  })

  test("keeps accessible content visible and neutralizes menu/landmark wrappers", () => {
    const markup = renderToStaticMarkup(
      <SideNav aria-label="产品导航" footerIcons={<strong>通知</strong>}>
        <SideNavSection heading="Workspace" endContent={<strong>2</strong>}>
          <SideNavItem label="Projects" startContent={<strong>2</strong>} icon={<strong>•</strong>} endContent={<strong>+</strong>} />
        </SideNavSection>
      </SideNav>,
    )

    expect(markup).toContain("通知")
    expect(markup).toContain("Workspace")
    expect(markup).toContain('role="presentation"')
    expect(markup).not.toContain('role="menu"')
    expect(markup).not.toContain("<aside")
  })

  test("only unmodified primary clicks are eligible for navigation callbacks", () => {
    const base = { button: 0, metaKey: false, ctrlKey: false, shiftKey: false, altKey: false, defaultPrevented: false }
    expect(isPrimaryNavigationClick(base)).toBe(true)
    for (const key of ["metaKey", "ctrlKey", "shiftKey", "altKey"] as const) {
      expect(isPrimaryNavigationClick({ ...base, [key]: true })).toBe(false)
    }
    expect(isPrimaryNavigationClick({ ...base, button: 1 })).toBe(false)
    expect(isPrimaryNavigationClick({ ...base, defaultPrevented: true })).toBe(false)
  })

  test("uses a bounded literal level map and one owned tree focus handler", () => {
    expect(Object.keys(TREE_LEVEL_CLASSES)).toEqual(["1", "2", "3", "4", "5", "6"])
    expect(Object.values(TREE_LEVEL_CLASSES).every((className) => /^ps-[0-9]+$/.test(className))).toBe(true)

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
    expect(source).not.toMatch(/<aside(?:\s|>)/)
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
    expect(source).not.toMatch(/(?:border-[rl]|text-left|\b(?:pl|pr|ml|mr)-)/)
    expect(source).toContain("text-primary")
    expect(source).toContain("text-secondary")
    expect(source).toContain("bg-surface")
    expect(source).toContain("border-border")
    expect(source).toContain("outline-accent")
  })
})
