import { expect, type Page, type Response } from "@playwright/test"

const strictCspDirectives = [
  "default-src 'self'",
  "script-src 'self'",
  "style-src 'self'",
  "img-src 'self' data:",
  "font-src 'self'",
  "connect-src 'self'",
  "object-src 'none'",
  "base-uri 'self'",
  "frame-ancestors 'none'",
] as const

export function expectStrictCsp(response: Response | null): void {
  expect(response).not.toBeNull()
  const csp = response?.headers()["content-security-policy"] ?? ""
  expect(csp).not.toBe("")
  expect(csp).not.toContain("unsafe-inline")
  for (const directive of strictCspDirectives) expect(csp).toContain(directive)
}

export async function expectNoPageOverflow(page: Page): Promise<void> {
  const widths = await page.evaluate(() => ({
    viewport: document.documentElement.clientWidth,
    document: document.documentElement.scrollWidth,
    body: document.body.scrollWidth,
  }))
  expect(widths.document).toBeLessThanOrEqual(widths.viewport)
  expect(widths.body).toBeLessThanOrEqual(widths.viewport)
}

export function expectTaskUrl(page: Page, pathname: string, display?: string): void {
  const url = new URL(page.url())
  expect(url.pathname).toBe(pathname)
  expect(url.searchParams.get("q")).toBe("agent")
  expect(url.searchParams.get("task")).toBe("t_default_ready")
  expect(url.searchParams.get("display")).toBe(display ?? null)
}
