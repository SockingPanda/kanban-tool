import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test, vi } from "vitest"

import {
  DateTimeInput,
  combineDateTimeValue,
  isISODate,
  isISOTime,
  parseDateTimeLocal,
  parseISODateTime,
} from "./DateTimeInput"

describe("CSP-safe Astryx DateTimeInput", () => {
  test("parses ISO and native local date/time values without timezone conversion", () => {
    expect(parseISODateTime("2026-08-09T09:10")).toEqual({ date: "2026-08-09", time: "09:10" })
    expect(parseISODateTime("2026-02-29T09:10")).toBeUndefined()
    expect(parseDateTimeLocal("2026-08-09", "09:10")).toBe("2026-08-09T09:10")
    expect(parseDateTimeLocal("2026-08-09T09:10")).toBe("2026-08-09T09:10")
    expect(combineDateTimeValue("2026-08-09", "09:10:30")).toBe("2026-08-09T09:10:30")
    expect(parseDateTimeLocal("2026-08-09", "")).toBeUndefined()
    expect(isISODate("2024-02-29")).toBe(true)
    expect(isISODate("2025-02-29")).toBe(false)
    expect(isISOTime("23:59")).toBe(true)
    expect(isISOTime("23:59:59")).toBe(true)
    expect(isISOTime("24:00")).toBe(false)
  })

  test("renders native date/time controls with Astryx-shaped field semantics", () => {
    const markup = renderToStaticMarkup(
      <DateTimeInput
        aria-describedby="external-help"
        autoComplete="off"
        data-testid="datetime-field"
        dateLabel="截止日期"
        hasClear
        htmlName="task-due-at"
        label="Due at"
        onChange={vi.fn()}
        status={{ type: "error", message: "Choose a valid deadline." }}
        timeIncrement={15}
        timeLabel="截止时间"
        clearLabel="清除截止时间"
        clearText="清除"
        translate="no"
        value="2026-08-09T09:10"
      />,
    )

    expect(markup).toContain('data-testid="datetime-field"')
    expect(markup).toContain('data-astryx-component="date-time-input"')
    expect(markup).toContain('type="date"')
    expect(markup).toContain('type="time"')
    expect(markup).toContain('name="task-due-at"')
    expect(markup).toContain('name="task-due-at-time"')
    expect(markup).toContain('value="2026-08-09"')
    expect(markup).toContain('value="09:10"')
    expect(markup).toContain('translate="no"')
    expect(markup).toContain('>截止日期</label>')
    expect(markup).toContain('>截止时间</label>')
    expect(markup).toContain('aria-label="清除截止时间"')
    expect(markup).toContain('>清除</button>')
    expect(markup).toContain('step="900"')
    expect(markup).toContain('aria-invalid="true"')
    expect(markup).toContain('Choose a valid deadline.')
    expect(markup).toContain('aria-label="清除截止时间"')
    expect(markup).not.toContain("style=")
    expect(markup).not.toContain("<style")
    expect(markup).not.toContain(`<${"div"}`)
    expect(markup).not.toContain(`<${"span"}`)
  })

  test("emits the combined ISO value as native segments change and preserves bounds", () => {
    const markup = renderToStaticMarkup(
      <DateTimeInput
        clearLabel="清除截止时间"
        dateLabel="截止日期"
        htmlName="task-due-at"
        label="截止时间"
        max="2026-08-30T18:00"
        min="2026-08-09T09:00"
        timeIncrement={15}
        timeLabel="时间"
        value="2026-08-30T09:00"
      />,
    )

    expect(markup).toContain('min="2026-08-09"')
    expect(markup).toContain('max="2026-08-30"')
    expect(markup).toContain('max="18:00"')
    expect(markup).toContain('step="900"')
    expect(markup).toContain('data-astryx-segment="date"')
    expect(markup).toContain('data-astryx-segment="time"')
  })

  test("keeps disabled and required semantics on both native controls", () => {
    const markup = renderToStaticMarkup(
      <DateTimeInput
        clearLabel="清除计划时间"
        dateLabel="计划日期"
        disabledMessage="Only owners can edit this field."
        isDisabled
        isRequired
        label="Scheduled at"
        timeLabel="计划时间"
        value="2026-08-09T09:10"
      />,
    )

    expect((markup.match(/disabled=""/g) ?? []).length).toBe(2)
    expect((markup.match(/required=""/g) ?? []).length).toBe(2)
    expect(markup).toContain('data-disabled="true"')
    expect(markup).toContain('aria-disabled="true"')
    expect(markup).toContain('Only owners can edit this field.')
  })

  test("does not expose a disabled message relation while the field is enabled", () => {
    const markup = renderToStaticMarkup(
      <DateTimeInput
        clearLabel="清除计划时间"
        dateLabel="计划日期"
        disabledMessage="Only owners can edit this field."
        label="Scheduled at"
        timeLabel="计划时间"
        value="2026-08-09T09:10"
      />,
    )

    expect(markup).not.toContain("Only owners can edit this field.")
    expect(markup).not.toContain("-disabled")
  })

  test("renders caller-provided optional copy without a default English suffix", () => {
    const markup = renderToStaticMarkup(
      <DateTimeInput
        clearLabel="清除时间"
        dateLabel="日期"
        isOptional
        label="计划时间"
        optionalText="（可选）"
        timeLabel="时间"
      />,
    )

    expect(markup).toContain("（可选）")
    expect(markup).not.toContain("optional")
  })
})
