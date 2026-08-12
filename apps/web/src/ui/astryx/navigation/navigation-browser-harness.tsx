import { useState } from "react"
import { createRoot } from "react-dom/client"

import { SideNav } from "./SideNav"
import { SideNavItem } from "./SideNavItem"
import { SideNavSection } from "./SideNavSection"
import { TreeList } from "./TreeList"
import type { TreeListItemData } from "./types"

declare global {
  interface Window {
    __setSafeNavCollapsed?: (collapsed: boolean) => void
  }
}

const treeItems: readonly TreeListItemData[] = [{
  id: "tree-root",
  label: "Tree root",
  children: [{ id: "tree-child", label: "Tree child", href: "#tree-child" }],
}]

export function NavigationBrowserHarness() {
  const [navCollapsed, setNavCollapsed] = useState(false)
  const [expandedIds, setExpandedIds] = useState<readonly string[]>(["tree-root"])
  const [actionCount, setActionCount] = useState(0)
  window.__setSafeNavCollapsed = setNavCollapsed

  return (
    <main>
      <SideNav
        aria-label="产品导航"
        collapsible={{ isCollapsed: navCollapsed, hasButton: true, expandLabel: "展开导航", collapseLabel: "收起导航" }}
      >
        <SideNavSection heading="First section">
          <SideNavItem label="First item" href="#first" data-testid="first-item" />
        </SideNavSection>
        <SideNavSection heading="Second section">
          <SideNavItem
            label="Parent item"
            href="#parent"
            aria-controls="parent-children"
            data-testid="parent-item"
            expandLabel="展开项目"
            collapseLabel="收起项目"
          >
            <SideNavItem label="Nested child" href="#nested" data-testid="nested-child" />
          </SideNavItem>
        </SideNavSection>
      </SideNav>
      <TreeList
        aria-label="Interaction tree"
        items={treeItems}
        expandedIds={expandedIds}
        onExpandedChange={(id, expanded) => setExpandedIds((current) => expanded ? [...current, id] : current.filter((value) => value !== id))}
        onAction={() => setActionCount((count) => count + 1)}
        expandLabel={(item) => `Expand ${item.id}`}
        collapseLabel={(item) => `Collapse ${item.id}`}
      />
      <output data-testid="tree-action-count">{actionCount}</output>
    </main>
  )
}

const root = document.getElementById("root")
if (root === null) throw new Error("navigation browser fixture root is missing")
createRoot(root).render(<NavigationBrowserHarness />)
