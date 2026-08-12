import { readFileSync } from "node:fs"
import { resolve } from "node:path"
import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test } from "vitest"

import {
  PageFrame,
  PAGE_FRAME_BODY_OVERFLOW_CLASSES,
  PAGE_FRAME_CLASSES,
} from "./PageFrame"

describe("CSP-safe PageFrame", () => {
  test("renders a padded content section with an article body", () => {
    const markup = renderToStaticMarkup(
      <PageFrame frame="content" aria-label="Settings">
        <p>Settings content</p>
      </PageFrame>,
    )

    expect(markup).toContain(`<section aria-label="Settings" class="${PAGE_FRAME_CLASSES.root.content}"`)
    expect(markup).toContain(`<article class="${PAGE_FRAME_CLASSES.body.content}"><p>Settings content</p></article>`)
    expect(markup).not.toContain("<main")
    expect(markup).not.toContain("style=")
  })

  test("keeps workspace chrome inset while its named body stays full-bleed", () => {
    const markup = renderToStaticMarkup(
      <PageFrame
        frame="workspace"
        id="tasks-frame"
        bodyLabel="Task board"
        bodyOverflow="x"
        header={<h1>Tasks</h1>}
        toolbarLabel="Task board controls"
        toolbar={<button type="button">Filter</button>}
        data-testid="page-frame"
      >
        <p>Dense task rows</p>
      </PageFrame>,
    )

    expect(markup).toContain(`id="tasks-frame"`)
    expect(markup).toContain(`data-testid="page-frame"`)
    expect(markup).toContain(`class="${PAGE_FRAME_CLASSES.root.workspace}"`)
    expect(markup).toContain(`<header class="${PAGE_FRAME_CLASSES.header.workspace}"><h1>Tasks</h1></header>`)
    expect(markup).toContain(`<section aria-label="Task board" class="${PAGE_FRAME_BODY_OVERFLOW_CLASSES.x}" data-body-overflow="x" role="region"><p>Dense task rows</p></section>`)
    expect(markup).toContain(`<section aria-label="Task board controls" class="${PAGE_FRAME_CLASSES.toolbar.workspace}" role="toolbar"><button type="button">Filter</button></section>`)
    expect(markup).toMatch(new RegExp(`<section id="tasks-frame" class="${PAGE_FRAME_CLASSES.root.workspace}"`))
    expect(markup).not.toContain("<main")
    expect(markup).not.toContain("style=")
  })

  test("supports labelled-by body regions without inventing labels", () => {
    const markup = renderToStaticMarkup(
      <PageFrame frame="workspace" bodyLabelledBy="task-heading">
        <h1 id="task-heading">Tasks</h1>
      </PageFrame>,
    )

    expect(markup).toContain('aria-labelledby="task-heading"')
    expect(markup).toContain('data-body-overflow="none"')
    expect(markup).toContain(`class="${PAGE_FRAME_BODY_OVERFLOW_CLASSES.none}"`)
  })

  test("source stays free of runtime style and raw layout escape hatches", () => {
    const sourceDirectory = resolve(import.meta.dirname)
    const source = ["PageFrame.tsx", "index.ts"]
      .map((file) => readFileSync(resolve(sourceDirectory, file), "utf8"))
      .join("\n")

    expect(source).not.toMatch(/<div|<span|<main|<style>/)
    expect(source).not.toMatch(/style\s*=|xstyle\b/)
    expect(source).not.toMatch(/className=\{[^}\n]*(?:\$\{|`)/)
    expect(source).not.toMatch(/(?:bg|text|border|p|m)-\[[^\]]+\]/)
    expect(source).toContain('export type PageFrameMode = "content" | "workspace"')
  })
})

// The safe frame intentionally rejects ambiguous semantic contracts.
// @ts-expect-error workspace bodies must expose exactly one accessible name.
const unnamedWorkspace = <PageFrame frame="workspace">Body</PageFrame>
// @ts-expect-error workspace bodies cannot provide both label forms.
const doublyNamedWorkspace = <PageFrame frame="workspace" bodyLabel="Body" bodyLabelledBy="body-id">Body</PageFrame>
// @ts-expect-error toolbar content must have a caller-owned accessible label.
const unnamedToolbar = <PageFrame frame="content" toolbar={<button type="button">Filter</button>}>Body</PageFrame>
// @ts-expect-error frame is a finite union.
const invalidFrame = <PageFrame frame="dashboard">Body</PageFrame>

void unnamedWorkspace
void doublyNamedWorkspace
void unnamedToolbar
void invalidFrame
