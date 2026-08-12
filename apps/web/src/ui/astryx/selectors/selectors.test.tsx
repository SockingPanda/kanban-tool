import { renderToStaticMarkup } from "react-dom/server"
import { readFileSync } from "node:fs"
import { resolve } from "node:path"
import type {ComponentProps} from "react"
import {describe, expect, test, vi} from "vitest"

import { MultiSelector } from "./MultiSelector"
import { createStaticSource } from "./search-source"
import { Selector } from "./Selector"
import { Typeahead } from "./Typeahead"

const options = [
  { value: "ready", label: "Ready" },
  { value: "review", label: "Review", disabled: true },
  { type: "section" as const, title: "More", options: [{ value: "done", label: "Done" }] },
]

describe("CSP-safe selector family", () => {
  test("keeps the unsearched Selector on native select semantics", () => {
    const markup = renderToStaticMarkup(
      <Selector
        label="Status"
        description="Choose a canonical task status"
        status={{ type: "warning", message: "Review is read-only" }}
        htmlName="status"
        options={options}
        value="ready"
        onChange={vi.fn()}
        placeholder="Choose a status"
        loadingText="Loading statuses"
        data-testid="status-selector"
      />,
    )

    expect(markup).toContain("<select")
    expect(markup).toContain('name="status"')
    expect(markup).toContain('data-selector-control="true"')
    expect(markup).toContain('<option value="">Choose a status</option>')
    expect(markup).toContain('data-testid="status-selector"')
    expect(markup).toContain('aria-describedby="selector-')
    expect(markup).toContain("Ready")
    expect(markup).toContain("More")
    expect(markup).toContain('disabled=""')
    expect(markup).not.toContain('style="')
    expect(markup).not.toContain("<div")
    expect(markup).not.toContain("<span")
  })

  test("sanitizes intrinsic spreads for every selector control", () => {
    const unsafe = {
      style: {color: "red"},
      xstyle: {color: "red"},
      dangerouslySetInnerHTML: {__html: "<style>body{color:red}</style>"},
      "data-testid": "sanitized-selector",
    }
    const selectorMarkup = renderToStaticMarkup(
      <Selector
        {...(unsafe as unknown as ComponentProps<typeof Selector>)}
        label="Status"
        options={["ready"]}
        value="ready"
        placeholder="Choose a status"
        loadingText="Loading statuses"
      />,
    )
    const multiMarkup = renderToStaticMarkup(
      <MultiSelector
        {...(unsafe as unknown as ComponentProps<typeof MultiSelector>)}
        label="Statuses"
        options={["ready"]}
        value={[]}
        onChange={vi.fn()}
        placeholder="Choose statuses"
        loadingText="Loading statuses"
        noOptionsText="Nothing"
        selectedText={(count) => `${count} statuses`}
      />,
    )
    const typeaheadMarkup = renderToStaticMarkup(
      <Typeahead
        {...(unsafe as unknown as ComponentProps<typeof Typeahead>)}
        label="Project"
        searchSource={createStaticSource([])}
        value={null}
        onChange={vi.fn()}
        placeholder="Find project"
        searchLabel="Find project"
        listboxLabel="Project matches"
        loadingText="Loading projects"
        clearLabel="Clear project"
        emptySearchResultsText="No project matches"
        errorText="Project search failed"
      />,
    )

    for (const markup of [selectorMarkup, multiMarkup, typeaheadMarkup]) {
      expect(markup).toContain('data-testid="sanitized-selector"')
      expect(markup).not.toContain('style=')
      expect(markup).not.toContain('xstyle')
      expect(markup).not.toContain('dangerouslySetInnerHTML')
      expect(markup).not.toContain('<style>')
    }
  })

  test("uses a same-wrapper static listbox and one hidden name per multi value", () => {
    const markup = renderToStaticMarkup(
      <MultiSelector
        label="Statuses"
        options={options}
        value={["ready", "done"]}
        onChange={vi.fn()}
        htmlName="status"
        placeholder="Choose statuses"
        loadingText="Fetching statuses"
        noOptionsText="Nothing"
        selectedText={(count) => `${count} statuses`}
        hasSelectAll
        selectAllLabel="Select everything"
        hasSearch
        searchLabel="Find statuses"
        searchPlaceholder="Filter statuses"
        data-testid="statuses-selector"
      />,
    )

    const openMarkup = renderToStaticMarkup(
      <MultiSelector
        label="Statuses"
        options={options}
        value={["ready", "done"]}
        onChange={vi.fn()}
        hasSearch
        isDefaultOpen
        placeholder="Choose statuses"
        loadingText="Fetching statuses"
        noOptionsText="Nothing"
        selectedText={(count) => `${count} statuses`}
        hasSelectAll
        selectAllLabel="Select everything"
        searchLabel="Find statuses"
        searchPlaceholder="Filter statuses"
      />,
    )

    expect(markup).not.toContain('role="combobox"')
    expect(markup).toContain('aria-haspopup="listbox"')
    expect(markup).toContain('aria-multiselectable="true"')
    expect(markup).toContain('name="status"')
    expect((markup.match(/name="status"/g) ?? []).length).toBe(2)
    expect(markup).toContain('type="hidden"')
    expect(markup).toContain('hidden=""')
    expect(openMarkup).toContain('role="combobox"')
    expect(openMarkup).not.toMatch(/<button[^>]*role="combobox"/)
    expect(markup).not.toContain('style="')
    expect(markup).not.toContain("<div")
    expect(markup).not.toContain("<span")
  })

  test("keeps select-all identity distinct and exposes caller-owned partial state", () => {
    const markup = renderToStaticMarkup(
      <MultiSelector
        label="Statuses"
        options={[
          { value: "__astryx_select_all__", label: "Collision" },
          { value: "done", label: "Done" },
        ]}
        value={["__astryx_select_all__"]}
        onChange={vi.fn()}
        placeholder="Choose statuses"
        loadingText="Fetching statuses"
        noOptionsText="Nothing"
        selectedText={(count) => `${count} statuses`}
        hasSelectAll
        selectAllLabel="Select everything"
        selectAllStateLabel={(state) => state === "some" ? "Some statuses selected" : state === "all" ? "All statuses selected" : "No statuses selected"}
      />,
    )

    expect(markup).toContain("Collision")
    expect(markup).toContain('aria-label="Select everything, Some statuses selected"')
    expect(markup).toContain('data-partial="true"')
  })

  test("exposes async SearchSource, selected value, and combobox IDREFs", () => {
    const items = [
      { id: "one", label: "One", auxiliaryData: { slug: "uno" } },
      { id: "two", label: "Two", auxiliaryData: { slug: "dos" } },
    ]
    const source = createStaticSource(items, { keywords: (item) => [item.auxiliaryData.slug] })
    expect(source.search("uno")).toEqual([items[0]])
    expect(source.bootstrap()).toEqual(items)

    const markup = renderToStaticMarkup(
      <Typeahead
        label="Project"
        searchSource={source}
        value={items[0]}
        onChange={vi.fn()}
        hasEntriesOnFocus
        debounceMs={0}
        placeholder="Find project"
        searchLabel="Find project"
        listboxLabel="Project matches"
        loadingText="Loading projects"
        clearLabel="Clear project"
        emptySearchResultsText="No project matches"
        errorText="Project search failed"
        isRequired
        required
        data-testid="project-typeahead"
      />,
    )

    expect(markup).toContain('role="combobox"')
    expect(markup).toContain('aria-controls="typeahead-')
    expect(markup).toContain('aria-expanded="false"')
    expect(markup).toContain('aria-required="true"')
    expect(markup).toContain('required=""')
    expect(markup).toContain("One")
    expect(markup).toContain('role="listbox"')
    expect(markup).toContain('hidden=""')
    expect(markup).not.toContain("aria-activedescendant")
    expect(markup).not.toContain('style="')
    expect(markup).not.toContain("<div")
    expect(markup).not.toContain("<span")
  })

  test("keeps disabled Typeahead native-disabled and gates its reason IDREF", () => {
    const markup = renderToStaticMarkup(
      <Typeahead
        label="Project"
        searchSource={createStaticSource([])}
        value={null}
        onChange={vi.fn()}
        isDisabled
        disabledMessage="Project access is unavailable"
        placeholder="Find project"
        searchLabel="Find project"
        listboxLabel="Project matches"
        loadingText="Loading projects"
        clearLabel="Clear project"
        emptySearchResultsText="No project matches"
        errorText="Project search failed"
      />,
    )

    expect(markup).toContain('disabled=""')
    expect(markup).not.toContain("readOnly")
    expect(markup).toContain("Project access is unavailable")
    expect(markup).toContain('aria-describedby="typeahead-')
  })

  test("does not create dangling IDREFs for empty copy", () => {
    const selectorMarkup = renderToStaticMarkup(
      <Selector
        label="Status"
        options={["ready"]}
        value="ready"
        description=""
        status={{ type: "warning", message: "" }}
        placeholder="Choose a status"
        loadingText="Loading statuses"
      />,
    )
    const typeaheadMarkup = renderToStaticMarkup(
      <Typeahead
        label="Project"
        searchSource={createStaticSource([])}
        value={null}
        onChange={vi.fn()}
        isDisabled
        disabledMessage=""
        placeholder="Find project"
        searchLabel="Find project"
        listboxLabel="Project matches"
        loadingText="Loading projects"
        clearLabel="Clear project"
        emptySearchResultsText="No project matches"
        errorText="Project search failed"
      />,
    )

    expect(selectorMarkup).not.toContain("aria-describedby")
    expect(selectorMarkup).not.toMatch(/id="[^"]*-(?:description|status)"/)
    expect(typeaheadMarkup).not.toContain("aria-describedby")
    expect(typeaheadMarkup).not.toMatch(/id="[^"]*-disabled-message"/)
  })

  test("does not expose forbidden style or overlay APIs in the source", () => {
    const markup = renderToStaticMarkup(
      <Selector label="Status" options={["ready"]} value="ready" onChange={vi.fn()} placeholder="Choose a status" loadingText="Loading statuses" />,
    )
    expect(markup).not.toContain("xstyle")
    expect(markup).not.toContain("width=")
    expect(markup).not.toContain("<style")
  })

  test("keeps the selector source free of runtime presentation and layout escapes", () => {
    const sourceDirectory = resolve(import.meta.dirname)
    const source = ["shared.ts", "search-source.ts", "Selector.tsx", "MultiSelector.tsx", "Typeahead.tsx", "index.ts"]
      .map((file) => readFileSync(resolve(sourceDirectory, file), "utf8"))
      .join("\n")

    expect(source).not.toMatch(/<div|<span|useLayer|usePopover|useTooltip|useAnnounce|<style>|\.style\b|xstyle\b/)
    expect(source).not.toMatch(/className=\{[^}\n]*(?:\$\{|`)/)
    expect(source).not.toMatch(/(?:bg|text|border|p|m)-\[[^\]]+\]/)
    expect(source).not.toMatch(/Select…|Search…|Loading…|No options|No results found|Unable to load results|>Clear<|\(optional\)/)
  })
})

// Safe selectors deliberately reject runtime presentation escape hatches.
// @ts-expect-error style is intentionally not part of the safe selector API.
const styleIsRejected = <Selector label="Status" options={["ready"]} placeholder="Choose a status" loadingText="Loading statuses" style={{}} />
// @ts-expect-error xstyle is intentionally not part of the safe selector API.
const xstyleIsRejected = <MultiSelector label="Status" options={["ready"]} value={[]} onChange={() => undefined} placeholder="Choose status" loadingText="Loading statuses" noOptionsText="Nothing" selectedText={(count) => `${count} statuses`} xstyle={{}} />
// @ts-expect-error width is intentionally not part of the safe selector API.
const widthIsRejected = <Typeahead label="Project" searchSource={createStaticSource([])} value={null} onChange={() => undefined} placeholder="Find project" searchLabel="Find project" listboxLabel="Project matches" loadingText="Loading projects" clearLabel="Clear project" emptySearchResultsText="No project matches" errorText="Project search failed" width="100%" />
// @ts-expect-error search-mode MultiSelector requires an accessible search label and placeholder.
const searchCopyIsRequired = <MultiSelector label="Status" options={["ready"]} value={[]} onChange={() => undefined} placeholder="Choose status" loadingText="Loading statuses" noOptionsText="Nothing" selectedText={() => "Status"} hasSearch />
// @ts-expect-error count-mode MultiSelector requires a caller-owned selected-count label.
const countCopyIsRequired = <MultiSelector label="Status" options={["ready"]} value={[]} onChange={() => undefined} placeholder="Choose status" loadingText="Loading statuses" noOptionsText="Nothing" />
// @ts-expect-error Typeahead copy is required for its control, listbox, loading, and result states.
const typeaheadCopyIsRequired = <Typeahead label="Project" searchSource={createStaticSource([])} value={null} onChange={() => undefined} />
// @ts-expect-error loading output requires caller-owned loading text.
const selectorLoadingCopyIsRequired = <Selector label="Status" options={["ready"]} placeholder="Choose a status" isLoading />
// @ts-expect-error native Selector does not support divider sentinel options.
const nativeDividerIsRejected = <Selector label="Status" options={[{ type: "divider" }]} placeholder="Choose a status" loadingText="Loading statuses" />
// @ts-expect-error native Selector does not support icon-bearing options.
const nativeIconIsRejected = <Selector label="Status" options={[{ value: "ready", icon: "icon" }]} placeholder="Choose a status" loadingText="Loading statuses" />
// @ts-expect-error native option rendering must return a string consumable by <option>.
const richNativeOptionIsRejected = <Selector label="Status" options={["ready"]} placeholder="Choose a status" loadingText="Loading statuses" renderOption={() => <strong>Ready</strong>} />
// @ts-expect-error required and optional labels are mutually exclusive.
const conflictingRequirementCopyIsRejected = <Selector label="Status" options={["ready"]} placeholder="Choose a status" loadingText="Loading statuses" isRequired isOptional optionalLabel="Optional" />

void styleIsRejected
void xstyleIsRejected
void widthIsRejected
void searchCopyIsRequired
void countCopyIsRequired
void typeaheadCopyIsRequired
void selectorLoadingCopyIsRequired
void nativeDividerIsRejected
void nativeIconIsRejected
void richNativeOptionIsRejected
void conflictingRequirementCopyIsRejected
