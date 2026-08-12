import {readFileSync} from "node:fs"
import {resolve} from "node:path"
import {renderToStaticMarkup} from "react-dom/server"
import {describe, expect, test} from "vitest"

import {CheckboxInput} from "./CheckboxInput"
import {FileInput} from "./FileInput"
import {TextArea} from "./TextArea"
import {TextInput} from "./TextInput"
import {mergeDescribedBy} from "./shared"

describe("CSP-safe field contracts", () => {
  test("deduplicates aria-describedby tokens without changing their order", () => {
    expect(mergeDescribedBy("caller help", "caller help generated", undefined, "generated")).toBe(
      "caller help generated",
    )
  })

  test("TextInput forwards native metadata and links every rendered description", () => {
    const markup = renderToStaticMarkup(
      <TextInput
        id="project-name"
        label="Project name"
        value="kanban"
        description="Use a stable name."
        status={{type: "error", message: "Name is already used."}}
        disabledMessage="Editing is locked."
        isDisabled
        htmlName="project"
        maxLength={40}
        autoComplete="off"
        translate="no"
        hasClear
        clearLabel="Reset project name"
        aria-describedby="caller-help project-name-description"
        data-testid="project-name"
        onChange={() => undefined}
      />,
    )
    const clearMarkup = renderToStaticMarkup(
      <TextInput
        label="Project name"
        value="kanban"
        hasClear
        clearLabel="Reset project name"
        onChange={() => undefined}
      />,
    )

    expect(markup).toContain('name="project"')
    expect(markup).toContain('maxLength="40"')
    expect(markup).toContain('autoComplete="off"')
    expect(markup).toContain('translate="no"')
    expect(clearMarkup).toContain('aria-label="Reset project name"')
    expect(markup).toContain('data-testid="project-name"')
    expect(markup).toContain(
      'aria-describedby="caller-help project-name-description project-name-status project-name-disabled"',
    )
    expect(markup).toContain('id="project-name-description"')
    expect(markup).toContain('id="project-name-status"')
    expect(markup).toContain('id="project-name-disabled"')
    expect(markup).toContain('aria-disabled="true"')
  })

  test("TextArea forwards maxLength, count ID, spellcheck, and status", () => {
    const markup = renderToStaticMarkup(
      <TextArea
        id="notes"
        label="Notes"
        value="abc"
        maxLength={10}
        description="Keep this concise."
        status={{type: "warning", message: "Almost complete."}}
        hasSpellCheck={false}
        autoComplete="off"
        translate="no"
        onChange={() => undefined}
      />,
    )

    expect(markup).toContain('maxLength="10"')
    expect(markup).toContain('spellCheck="false"')
    expect(markup).toContain('autoComplete="off"')
    expect(markup).toContain('translate="no"')
    expect(markup).toContain('id="notes-counter"')
    expect(markup).toContain('aria-describedby="notes-description notes-status notes-counter"')
    expect(markup).toContain("3/10")
  })

  test("CheckboxInput and FileInput retain native names, refs, and file constraints", () => {
    const checkboxMarkup = renderToStaticMarkup(
      <CheckboxInput
        id="terms"
        label="Terms"
        value="indeterminate"
        htmlName="terms"
        description="Required to continue."
        isRequired
        autoComplete="off"
        translate="no"
        onChange={() => undefined}
      />,
    )
    const fileMarkup = renderToStaticMarkup(
      <FileInput
        id="attachment"
        label="Attachment"
        value={null}
        htmlName="attachment"
        accept="image/*"
        isMultiple
        maxSize={1024}
        maxFiles={2}
        autoComplete="off"
        translate="no"
        onChange={() => undefined}
      />,
    )

    expect(checkboxMarkup).toContain('type="checkbox"')
    expect(checkboxMarkup).toContain('name="terms"')
    expect(checkboxMarkup).toContain('autoComplete="off"')
    expect(checkboxMarkup).toContain('translate="no"')
    expect(checkboxMarkup).toContain('aria-describedby="terms-description"')
    expect(fileMarkup).toContain('type="file"')
    expect(fileMarkup).toContain('name="attachment"')
    expect(fileMarkup).toContain('accept="image/*"')
    expect(fileMarkup).toContain('multiple=""')
    expect(fileMarkup).toContain('autoComplete="off"')
    expect(fileMarkup).toContain('translate="no"')
  })

  test("static markup never emits runtime style attributes or layout div/span", () => {
    const sourceDirectory = resolve(import.meta.dirname)
    const source = ["shared.tsx", "TextInput.tsx", "TextArea.tsx", "CheckboxInput.tsx", "FileInput.tsx"]
      .map((file) => readFileSync(resolve(sourceDirectory, file), "utf8"))
      .join("\n")
    const markup = [
      renderToStaticMarkup(<TextInput label="Name" value="" onChange={() => undefined} />),
      renderToStaticMarkup(<TextArea label="Notes" value="" onChange={() => undefined} />),
      renderToStaticMarkup(<CheckboxInput label="Terms" value={false} onChange={() => undefined} />),
      renderToStaticMarkup(<FileInput label="File" value={null} onChange={() => undefined} />),
    ].join("\n")

    expect(source).not.toMatch(/<div|<span|<style>/)
    expect(markup).not.toMatch(/ style=/)
    expect(markup).not.toMatch(/<div|<span/)
    expect(markup).not.toMatch(/class="[^"]*\[/)
    expect(source).toContain(
      'onChange?.("", null as unknown as ChangeEvent<HTMLInputElement>)',
    )
    expect(source).toContain("inputRef.current?.focus()")
  })
})

// The safe surface intentionally rejects runtime presentation escape hatches.
// @ts-expect-error style must not be accepted by a safe field.
const styleIsRejected = <TextInput label="Name" value="" onChange={() => undefined} style={{}} />
// @ts-expect-error xstyle must not be accepted by a safe field.
const xstyleIsRejected = <TextInput label="Name" value="" onChange={() => undefined} xstyle={{}} />
// @ts-expect-error width must not be accepted by a safe field.
const widthIsRejected = <TextInput label="Name" value="" onChange={() => undefined} width="100%" />
// @ts-expect-error style must not be accepted by a safe field.
const textAreaStyleIsRejected = <TextArea label="Notes" value="" onChange={() => undefined} style={{}} />
// @ts-expect-error xstyle must not be accepted by a safe field.
const checkboxXstyleIsRejected = <CheckboxInput label="Terms" value={false} onChange={() => undefined} xstyle={{}} />
// @ts-expect-error width must not be accepted by a safe field.
const fileWidthIsRejected = <FileInput label="File" value={null} onChange={() => undefined} width="100%" />

void styleIsRejected
void xstyleIsRejected
void widthIsRejected
void textAreaStyleIsRejected
void checkboxXstyleIsRejected
void fileWidthIsRejected
