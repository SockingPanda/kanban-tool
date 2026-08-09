import { chmod, lstat, mkdir, open, rename, unlink } from "node:fs/promises"

import AxeBuilder from "@axe-core/playwright"
import { expect, type Page, type TestInfo } from "@playwright/test"

type Impact = "critical" | "serious" | "moderate" | "minor" | null

export type AxeIncompleteDisposition = {
  readonly reviewed: true
  readonly result: "not-applicable" | "manual-pass"
  readonly rationale: string
}

export type AxeCheckEvidence = {
  readonly id: string
  readonly impact: string
  readonly message: string
  readonly data: unknown
}

export type AxeComputedEvidence = {
  readonly accessible_name: string | null
  readonly foreground: string | null
  readonly background: string | null
  readonly font_size: string | null
  readonly font_weight: string | null
  readonly contrast_ratio: number | null
  readonly disabled: boolean | null
  readonly opacity: string | null
  readonly error: string | null
}

type AxeNodeEvidence = {
  readonly target: string
  readonly html: string
  readonly failureSummary: string | null
  readonly any: readonly AxeCheckEvidence[]
  readonly all: readonly AxeCheckEvidence[]
  readonly none: readonly AxeCheckEvidence[]
  readonly computed: AxeComputedEvidence | null
}

export type AxeEvidence = {
  readonly label: string
  readonly url: string
  readonly context: BrowserContextObservation
  readonly impactCounts: Record<"critical" | "serious" | "moderate" | "minor" | "unknown", number>
  readonly violations: readonly {
    readonly id: string
    readonly impact: Impact
    readonly help: string
    readonly helpUrl: string
    readonly nodes: readonly AxeNodeEvidence[]
  }[]
  readonly passes: number
  readonly incomplete: readonly {
    readonly id: string
    readonly help: string
    readonly helpUrl: string
    readonly nodes: readonly AxeNodeEvidence[]
    readonly disposition: AxeIncompleteDisposition | null
  }[]
  readonly inapplicable: number
}

export type BrowserContextObservation = {
  readonly document_language: string
  readonly navigator_language: string
  readonly intl_locale: string
  readonly color_scheme_light: boolean
  readonly color_scheme_dark: boolean
  readonly reduced_motion_reduce: boolean
  readonly theme_state: string
  readonly astryx_theme: string | null
  readonly computed_color_scheme: string
}

export type KeyboardEvidence = {
  readonly label: string
  readonly focusVisible: boolean
  readonly outlineWidth: string
  readonly outlineStyle: string
  readonly outlineColor: string
  readonly boxShadow: string
}

const evidenceDirectory = process.env.KANBAN_A11Y_EVIDENCE_DIR ?? "../../output/release/a11y-09c"
const runId = process.env.KANBAN_A11Y_RUN_ID ?? "local-a11y-09c"

export const evidenceContext = {
  locale: "zh-CN",
  theme: "light",
  reduced_motion: "reduce",
} as const

export function hostIdentityEvidence(): {
  readonly pid: string | null
  readonly start_time: string | null
  readonly listener_owner_pid: string | null
  readonly listener_owner_start_time: string | null
  readonly exe: string | null
  readonly sha256: string | null
  readonly argv: string | null
  readonly port: string | null
  readonly web_dir: string | null
  readonly mode: string | null
  readonly start_sha: string | null
  readonly end_sha: string | null
  readonly start_clean: string | null
  readonly end_clean: string | null
  readonly db: "turso"
  readonly db_path: string | null
} {
  return {
    pid: process.env.KANBAN_A11Y_HOST_PID ?? null,
    start_time: process.env.KANBAN_A11Y_HOST_START_TIME ?? null,
    listener_owner_pid: process.env.KANBAN_A11Y_LISTENER_OWNER_PID ?? null,
    listener_owner_start_time: process.env.KANBAN_A11Y_LISTENER_OWNER_START_TIME ?? null,
    exe: process.env.KANBAN_A11Y_HOST_EXE ?? null,
    sha256: process.env.KANBAN_A11Y_KANBAN_SHA256 ?? null,
    argv: process.env.KANBAN_A11Y_HOST_ARGV ?? null,
    port: process.env.KANBAN_A11Y_PORT ?? null,
    web_dir: process.env.KANBAN_A11Y_WEB_DIR ?? null,
    mode: process.env.KANBAN_A11Y_MODE ?? null,
    start_sha: process.env.KANBAN_A11Y_START_SHA ?? null,
    end_sha: process.env.KANBAN_A11Y_END_SHA ?? null,
    start_clean: process.env.KANBAN_A11Y_START_CLEAN ?? null,
    end_clean: process.env.KANBAN_A11Y_END_CLEAN ?? null,
    db: "turso",
    db_path: process.env.KANBAN_A11Y_DB_PATH ?? null,
  }
}

export function evidenceRunId(): string {
  return runId
}

export async function observeBrowserContext(page: Page): Promise<BrowserContextObservation> {
  return page.evaluate(() => ({
    document_language: document.documentElement.lang,
    navigator_language: navigator.language,
    intl_locale: Intl.DateTimeFormat().resolvedOptions().locale,
    color_scheme_light: window.matchMedia("(prefers-color-scheme: light)").matches,
    color_scheme_dark: window.matchMedia("(prefers-color-scheme: dark)").matches,
    reduced_motion_reduce: window.matchMedia("(prefers-reduced-motion: reduce)").matches,
    theme_state: document.documentElement.dataset.theme ?? "system",
    astryx_theme: document.querySelector("[data-astryx-theme]")?.getAttribute("data-astryx-theme") ?? null,
    computed_color_scheme: getComputedStyle(document.documentElement).colorScheme,
  }))
}

function checkEvidence(check: { id: string; impact: string; message: string; data: unknown }): AxeCheckEvidence {
  return {
    id: check.id,
    impact: check.impact,
    message: check.message,
    data: check.data,
  }
}

async function computedEvidence(page: Page, selector: string): Promise<AxeComputedEvidence> {
  return page.evaluate((target) => {
    const element = document.querySelector<HTMLElement>(target)
    if (!element) return { accessible_name: null, foreground: null, background: null, font_size: null, font_weight: null, contrast_ratio: null, disabled: null, opacity: null, error: `selector not found: ${target}` }
    const parse = (value: string): [number, number, number, number] | null => {
      const match = value.match(/^rgba?\(([^)]+)\)$/)
      if (!match) return null
      const parts = match[1].split(",").map((part) => part.trim())
      if (parts.length < 3) return null
      const channels = parts.slice(0, 3).map(Number)
      if (channels.some((channel) => !Number.isFinite(channel))) return null
      const alpha = parts.length === 4 ? Number(parts[3]) : 1
      return [channels[0], channels[1], channels[2], Number.isFinite(alpha) ? alpha : 1]
    }
    const composite = (foreground: [number, number, number, number], background: [number, number, number, number]): [number, number, number, number] => {
      const alpha = foreground[3] + background[3] * (1 - foreground[3])
      if (alpha === 0) return [0, 0, 0, 0]
      return [
        (foreground[0] * foreground[3] + background[0] * background[3] * (1 - foreground[3])) / alpha,
        (foreground[1] * foreground[3] + background[1] * background[3] * (1 - foreground[3])) / alpha,
        (foreground[2] * foreground[3] + background[2] * background[3] * (1 - foreground[3])) / alpha,
        alpha,
      ]
    }
    const luminance = (color: [number, number, number, number]): number => {
      const channels = color.slice(0, 3).map((channel) => {
        const normalized = channel / 255
        return normalized <= 0.03928 ? normalized / 12.92 : ((normalized + 0.055) / 1.055) ** 2.4
      })
      return channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722
    }
    const style = getComputedStyle(element)
    const foreground = parse(style.color)
    let background: [number, number, number, number] | null = null
    let current: HTMLElement | null = element
    while (current) {
      const candidate = parse(getComputedStyle(current).backgroundColor)
      if (candidate && candidate[3] > 0) {
        background = background ? composite(candidate, background) : candidate
        if (background[3] >= 0.999) break
      }
      current = current.parentElement
    }
    if (!background) background = [255, 255, 255, 1]
    const effectiveForeground = foreground ? composite(foreground, background) : null
    const ratio = effectiveForeground ? (Math.max(luminance(effectiveForeground), luminance(background)) + 0.05) / (Math.min(luminance(effectiveForeground), luminance(background)) + 0.05) : null
    return {
      accessible_name: element.getAttribute("aria-label") ?? element.textContent?.trim() ?? null,
      foreground: style.color,
      background: `rgba(${background[0]}, ${background[1]}, ${background[2]}, ${background[3]})`,
      font_size: style.fontSize,
      font_weight: style.fontWeight,
      contrast_ratio: ratio,
      disabled: element.matches(":disabled"),
      opacity: style.opacity,
      error: null,
    }
  }, selector)
}

async function nodeEvidence(page: Page, node: { target: readonly unknown[]; html: string; failureSummary?: string; any: readonly { id: string; impact: string; message: string; data: unknown }[]; all: readonly { id: string; impact: string; message: string; data: unknown }[]; none: readonly { id: string; impact: string; message: string; data: unknown }[] }): Promise<AxeNodeEvidence> {
  const target = node.target.map((value) => typeof value === "string" ? value : JSON.stringify(value)).join(", ")
  return {
    target,
    html: node.html,
    failureSummary: node.failureSummary ?? null,
    any: node.any.map(checkEvidence),
    all: node.all.map(checkEvidence),
    none: node.none.map(checkEvidence),
    computed: await computedEvidence(page, target),
  }
}

function impactKey(impact: Impact): keyof AxeEvidence["impactCounts"] {
  return impact ?? "unknown"
}

export async function auditAxe(page: Page, label: string, evidence: AxeEvidence[], dispositionFor?: (label: string, ruleId: string) => AxeIncompleteDisposition | null): Promise<void> {
  const result = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22a", "wcag22aa"])
    .analyze()
  const impactCounts: AxeEvidence["impactCounts"] = {
    critical: 0,
    serious: 0,
    moderate: 0,
    minor: 0,
    unknown: 0,
  }
  for (const violation of result.violations) impactCounts[impactKey(violation.impact as Impact)] += 1
  const violations = await Promise.all(result.violations.map(async (violation) => ({
    id: violation.id,
    impact: violation.impact as Impact,
    help: violation.help,
    helpUrl: violation.helpUrl,
    nodes: await Promise.all(violation.nodes.map((node) => nodeEvidence(page, node))),
  })))
  const incomplete = await Promise.all(result.incomplete.map(async (rule) => ({
    id: rule.id,
    help: rule.help,
    helpUrl: rule.helpUrl,
    nodes: await Promise.all(rule.nodes.map((node) => nodeEvidence(page, node))),
    disposition: dispositionFor?.(label, rule.id) ?? null,
  })))
  evidence.push({
    label,
    url: page.url(),
    context: await observeBrowserContext(page),
    impactCounts,
    violations,
    passes: result.passes.length,
    incomplete,
    inapplicable: result.inapplicable.length,
  })
  expect(result.violations, `${label}: axe WCAG violations`).toEqual([])
}

export async function reachByKeyboard(page: Page, locator: ReturnType<Page["locator"]>, label: string, maxTabs = 80): Promise<void> {
  const target = locator.first()
  await page.evaluate(() => {
    const active = document.activeElement
    if (active instanceof HTMLElement) active.blur()
    // Firefox 会从先前聚焦的子树继续顺序导航；临时聚焦 body 可回到文档起点。
    document.body.setAttribute("tabindex", "-1")
    document.body.focus({ preventScroll: true })
  })
  try {
    for (let index = 0; index < maxTabs; index += 1) {
      await page.keyboard.press("Tab")
      if (await target.evaluate((element) => element === document.activeElement)) return
    }
    throw new Error(`${label}: keyboard Tab traversal did not reach target within ${maxTabs} steps`)
  } finally {
    await page.evaluate(() => document.body.removeAttribute("tabindex"))
  }
}

export async function expectFocusVisible(page: Page, label: string, keyboardEvidence: KeyboardEvidence[], locator: ReturnType<Page["locator"]>): Promise<void> {
  const target = locator.first()
  await reachByKeyboard(page, target, label)
  await expect(target).toBeFocused()
  const style = await target.evaluate((element) => {
    const computed = getComputedStyle(element)
    return {
      focusVisible: element.matches(":focus-visible"),
      outlineWidth: computed.outlineWidth,
      outlineStyle: computed.outlineStyle,
      outlineColor: computed.outlineColor,
      boxShadow: computed.boxShadow,
    }
  })
  expect(style.focusVisible, `${label}: :focus-visible`).toBe(true)
  expect(style.outlineWidth !== "0px" || style.boxShadow !== "none", `${label}: visible focus indicator`).toBe(true)
  keyboardEvidence.push({ label, ...style })
}

export async function writeEvidence(testInfo: TestInfo, kind: "keyboard" | "visual", value: unknown): Promise<void> {
  await mkdir(evidenceDirectory, { recursive: true })
  await chmod(evidenceDirectory, 0o700)
  const directory = await lstat(evidenceDirectory)
  if (!directory.isDirectory() || (directory.mode & 0o777) !== 0o700) throw new Error(`evidence dir must be private: ${evidenceDirectory}`)
  const destination = `${evidenceDirectory}/${kind}-${testInfo.project.name}.json`
  const destinationStat = await lstat(destination).catch(() => null)
  if (destinationStat?.isSymbolicLink()) throw new Error(`evidence destination is symlink: ${destination}`)
  const temporary = `${destination}.${process.pid}.${Date.now()}.tmp`
  let committed = false
  try {
    const handle = await open(temporary, "wx", 0o600)
    try {
      await handle.writeFile(`${JSON.stringify(value, null, 2)}\n`, "utf8")
      await handle.sync()
    } finally {
      await handle.close()
    }
    await rename(temporary, destination)
    committed = true
    const destinationStatAfterRename = await lstat(destination)
    if (!destinationStatAfterRename.isFile() || destinationStatAfterRename.nlink !== 1 || (destinationStatAfterRename.mode & 0o777) !== 0o600) {
      throw new Error(`evidence destination must be regular, single-link, mode 600: ${destination}`)
    }
  } finally {
    if (!committed) await unlink(temporary).catch(() => undefined)
  }
}

export function screenshotMaskSelectors(selectors: readonly string[]): readonly string[] {
  return [...selectors]
}
