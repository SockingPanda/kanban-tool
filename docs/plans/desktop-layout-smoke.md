# Desktop Layout Scroll Smoke

This checklist complements the automatic Vitest layout contracts in
`apps/desktop/src/app/layout-scroll-contract.test.ts`. Use it before visual
scrollbar fade work or shell layout changes that affect board, sheet, or sidebar
overflow.

## Automatic contracts

- Board horizontal overflow stays on the board scroller, with vertical overflow hidden at that level.
- Board columns keep `min-h-0`, `overflow-hidden`, and a vertical `ScrollArea` body.
- Task detail sheet content remains fixed/flex, while `TaskDetail` owns the body scroll area.
- Sidebar width transitions clip content and hide expanded labels until the width transition finishes.

## Manual narrow-width smoke

Run the desktop web shell at a narrow browser width, for example 390px to 480px,
against a local API runtime with enough tasks to overflow a board column.

- Board horizontal overflow: the board scrolls horizontally, the page itself does not create a second horizontal scrollbar, and the final column remains reachable.
- Column vertical scroll: a tall column scrolls inside its column body; the column header remains visible and adjacent columns do not grow taller than the viewport.
- Task detail sheet body scroll: opening a task detail sheet keeps the sheet header visible, scrolls the detail body, and keeps the right edge within `100vw - 32px`.
- Sidebar transition and clipping: collapsing and expanding the sidebar clips labels during the transition, icon buttons stay usable, and labels do not overlap the main header at narrow widths.
