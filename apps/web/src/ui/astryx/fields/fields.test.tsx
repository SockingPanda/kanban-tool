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
        clearText="Reset"
        requiredText="Required"
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
        clearText="Reset"
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
        optionalText="Optional"
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
        requiredText="Required"
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
        chooseFileText="Choose one"
        chooseFilesText="Choose many"
        clearLabel="Remove attachment"
        clearText="Remove"
        invalidTypeMessage={(file) => `${file.name} is not allowed`}
        sizeLimitMessage={(file, maxSize, formattedSize) => `${file.name} exceeds ${formattedSize}/${maxSize}`}
        maxFilesMessage={(maxFiles) => `At most ${maxFiles}`}
        formatFileSize={(bytes) => `${bytes} bytes`}
        autoComplete="off"
        translate="no"
        onChange={() => undefined}
      />,
    )
    const selectedFileMarkup = renderToStaticMarkup(
      <FileInput
        id="selected-file"
        label="Selected file"
        value={{name: "notes.txt"} as File}
        chooseFileText="Choose one"
        chooseFilesText="Choose many"
        clearLabel="Remove attachment"
        clearText="Remove"
        invalidTypeMessage="Invalid file"
        sizeLimitMessage="File too large"
        maxFilesMessage="Too many files"
        formatFileSize={(bytes) => `${bytes} bytes`}
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
    expect(fileMarkup).toContain("Choose many")
    expect(selectedFileMarkup).toContain('aria-describedby="selected-file-file-names"')
    expect(selectedFileMarkup).toContain('id="selected-file-file-names"')
    expect(selectedFileMarkup).toContain("notes.txt")
  })

  test("keeps caller-owned copy visible and does not invent English defaults", () => {
    const localized = renderToStaticMarkup(
      <TextInput
        label="名称"
        value="value"
        isRequired
        requiredText="必填"
        hasClear
        clearLabel="重置"
        clearText="重置"
        onChange={() => undefined}
      />,
    )
    const defaults = renderToStaticMarkup(
      <FileInput
        label="文件"
        value={null}
        chooseFileText="选择文件"
        chooseFilesText="选择文件"
        clearLabel="移除文件"
        clearText="移除"
        invalidTypeMessage="文件类型不支持"
        sizeLimitMessage="文件过大"
        maxFilesMessage="文件过多"
        formatFileSize={(bytes) => `${bytes} 字节`}
        onChange={() => undefined}
      />,
    )

    expect(localized).toContain("必填")
    expect(localized).toContain("重置")
    expect(defaults).not.toMatch(/Required|Optional|Clear|Choose|Maximum|exceeds|accepted file/i)
  })

  test("keeps a hidden label's description visible and uses native disabled controls", () => {
    const textInputMarkup = renderToStaticMarkup(
      <TextInput
        id="locked"
        label="Locked"
        value="value"
        isLabelHidden
        description="Still visible help"
        isDisabled
        disabledMessage="Locked by policy"
        onChange={() => undefined}
      />,
    )
    const checkboxMarkup = renderToStaticMarkup(
      <CheckboxInput
        id="locked-check"
        label="Locked check"
        value
        isDisabled
        disabledMessage="Locked by policy"
        onChange={() => undefined}
      />,
    )

    expect(textInputMarkup).toContain('id="locked-description"')
    expect(textInputMarkup).toContain('disabled=""')
    expect(textInputMarkup).not.toContain('readOnly=""')
    expect(textInputMarkup).toContain("Still visible help")
    expect(textInputMarkup).not.toContain('id="locked-description" class="sr-only')
    expect(checkboxMarkup).toContain('disabled=""')
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
      renderToStaticMarkup(
        <FileInput
          label="File"
          value={null}
          chooseFileText="Choose file"
          chooseFilesText="Choose files"
          clearLabel="Remove file"
          clearText="Remove"
          invalidTypeMessage="Invalid file"
          sizeLimitMessage="File too large"
          maxFilesMessage="Too many files"
          formatFileSize={(bytes) => `${bytes} bytes`}
          onChange={() => undefined}
        />,
      ),
    ].join("\n")

    expect(source).not.toMatch(/<div|<span|<style>/)
    expect(source).not.toContain("labelTooltip")
    expect(source).not.toContain("labelIcon")
    expect(source).not.toContain("startIcon")
    expect(markup).not.toMatch(/ style=/)
    expect(markup).not.toMatch(/<div|<span/)
    expect(markup).not.toMatch(/class="[^"]*\[/)
    expect(source).toContain(
      'onChange?.("", null)',
    )
    expect(source).not.toContain("as unknown as ChangeEvent")
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
const fileWidthIsRejected = (
  <FileInput
    label="File"
    value={null}
    chooseFileText="Choose file"
    chooseFilesText="Choose files"
    clearLabel="Remove file"
    clearText="Remove"
    invalidTypeMessage="Invalid file"
    sizeLimitMessage="File too large"
    maxFilesMessage="Too many files"
    formatFileSize={(bytes) => `${bytes} bytes`}
    onChange={() => undefined}
    // @ts-expect-error width must not be accepted by a safe field.
    width="100%"
  />
)
// @ts-expect-error clearLabel and clearText are required when hasClear is true.
const clearCopyIsRequired = <TextInput label="Name" value="x" hasClear onChange={() => undefined} />
// @ts-expect-error labelTooltip is not part of the safe field API.
const labelTooltipIsRejected = <TextInput label="Name" value="" labelTooltip="help" />
// @ts-expect-error startIcon is not part of the safe field API.
const startIconIsRejected = <TextInput label="Name" value="" startIcon="icon" />
// @ts-expect-error only the attached status variant is supported.
const detachedStatusIsRejected = <TextInput label="Name" value="" statusVariant="detached" />
// @ts-expect-error tooltip status is not a safe presentation API.
const tooltipStatusIsRejected = <TextInput label="Name" value="" statusVariant="tooltip" />
const modeApiIsRejected = (
  <FileInput
    label="File"
    value={null}
    chooseFileText="Choose file"
    chooseFilesText="Choose files"
    clearLabel="Remove file"
    clearText="Remove"
    invalidTypeMessage="Invalid file"
    sizeLimitMessage="File too large"
    maxFilesMessage="Too many files"
    formatFileSize={(bytes) => `${bytes} bytes`}
    onChange={() => undefined}
    // @ts-expect-error dropzone mode is intentionally not exposed.
    mode="dropzone"
  />
)
const dropHandlerApiIsRejected = (
  <FileInput
    label="File"
    value={null}
    chooseFileText="Choose file"
    chooseFilesText="Choose files"
    clearLabel="Remove file"
    clearText="Remove"
    invalidTypeMessage="Invalid file"
    sizeLimitMessage="File too large"
    maxFilesMessage="Too many files"
    formatFileSize={(bytes) => `${bytes} bytes`}
    onChange={() => undefined}
    // @ts-expect-error native drop handlers are intentionally not exposed.
    onDrop={() => undefined}
  />
)

void styleIsRejected
void xstyleIsRejected
void widthIsRejected
void textAreaStyleIsRejected
void checkboxXstyleIsRejected
void fileWidthIsRejected
void clearCopyIsRequired
void labelTooltipIsRejected
void startIconIsRejected
void detachedStatusIsRejected
void tooltipStatusIsRejected
void modeApiIsRejected
void dropHandlerApiIsRejected
