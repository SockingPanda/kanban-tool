import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, it } from "vitest"

import { MarkdownDescription } from "./TaskDetail"

describe("MarkdownDescription", () => {
  it("renders GFM markdown without enabling raw HTML", () => {
    const html = renderToStaticMarkup(
      <MarkdownDescription>{"**bold**\n\n- item\n\n<script>alert('x')</script>"}</MarkdownDescription>,
    )

    expect(html).toContain("<strong>bold</strong>")
    expect(html).toContain("<li>item</li>")
    expect(html).not.toContain("<script>")
    expect(html).toContain("&lt;script&gt;")
  })

  it("renders links as external links and filters unsafe protocols", () => {
    const html = renderToStaticMarkup(
      <MarkdownDescription>{"[safe](https://example.com) [bad](javascript:alert('x'))"}</MarkdownDescription>,
    )

    expect(html).toContain('href="https://example.com"')
    expect(html).toContain('target="_blank"')
    expect(html).toContain('rel="noreferrer noopener"')
    expect(html).not.toContain('href="javascript:alert')
  })
})
