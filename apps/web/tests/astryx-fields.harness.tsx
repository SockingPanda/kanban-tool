import {useState} from "react"
import {createRoot} from "react-dom/client"

import {FileInput} from "../src/ui/astryx/fields/FileInput"
import {TextArea} from "../src/ui/astryx/fields/TextArea"
import {TextInput} from "../src/ui/astryx/fields/TextInput"

interface FieldLogEntry {
  readonly field: string
  readonly kind: string
  readonly value?: string
  readonly reason?: string
}

declare global {
  interface Window {
    __astryxFieldLogs: FieldLogEntry[]
    __astryxFieldRef: HTMLInputElement | null
  }
}

const fieldLogs: FieldLogEntry[] = []
window.__astryxFieldLogs = fieldLogs
window.__astryxFieldRef = null

function log(entry: FieldLogEntry): void {
  fieldLogs.push(entry)
  const output = document.querySelector<HTMLOutputElement>("#field-log")
  if (output) {
    output.value = JSON.stringify(fieldLogs)
  }
}

const fileCopy = {
  chooseFileText: "Choose file",
  chooseFilesText: "Choose files",
  clearLabel: "Clear files",
  clearText: "Clear",
  invalidTypeMessage: (file: File) => `${file.name} is not accepted`,
  sizeLimitMessage: (file: File, maxSize: number, formattedSize: string) =>
    `${file.name} exceeds ${formattedSize}/${maxSize}`,
  maxFilesMessage: (maxFiles: number) => `At most ${maxFiles} files`,
  formatFileSize: (bytes: number) => `${bytes} bytes`,
}

function oneFile(files: File | File[] | null): File | null {
  return Array.isArray(files) ? files[0] ?? null : files
}

export function FieldHarness() {
  const [text, setText] = useState("draft")
  const [selectedFile, setSelectedFile] = useState<File | null>(null)

  return (
    <main>
      <TextInput
        id="text-clear"
        label="Text value"
        value={text}
        status={{type: "success", message: "Saved."}}
        hasClear
        clearLabel="Clear text value"
        clearText="Clear"
        onChange={(value, event) => {
          log({field: "text-clear", kind: "change", value, reason: event === null ? "null" : "event"})
          setText(value)
        }}
      />
      <TextArea
        id="notes-status"
        label="Notes"
        value=""
        status={{type: "warning", message: "Review notes."}}
        onChange={() => undefined}
      />

      <FileInput
        {...fileCopy}
        id="valid-file"
        ref={(node) => {
          window.__astryxFieldRef = node
        }}
        label="Valid file"
        value={selectedFile}
        isMultiple={false}
        onChange={(files) => {
          const nextFile = oneFile(files)
          log({field: "valid-file", kind: "change", value: nextFile?.name ?? "null"})
          setSelectedFile(nextFile)
        }}
        changeAction={(files) => {
          const nextFile = oneFile(files)
          log({field: "valid-file", kind: "action", value: nextFile?.name ?? "null"})
        }}
      />

      <FileInput
        {...fileCopy}
        id="atomic-file"
        label="Atomic file"
        value={null}
        accept="text/plain"
        isMultiple
        onChange={(files) => {
          log({field: "atomic-file", kind: "change", value: oneFile(files)?.name ?? "null"})
        }}
        onValidationError={(error) => {
          log({field: "atomic-file", kind: "error", reason: error.reason})
        }}
        changeAction={() => {
          log({field: "atomic-file", kind: "action"})
        }}
      />

      <FileInput
        {...fileCopy}
        id="max-files-file"
        label="Maximum files"
        value={null}
        isMultiple
        maxFiles={1}
        onChange={(files) => {
          log({field: "max-files-file", kind: "change", value: oneFile(files)?.name ?? "null"})
        }}
        onValidationError={(error) => {
          log({field: "max-files-file", kind: "error", reason: error.reason})
        }}
        changeAction={() => {
          log({field: "max-files-file", kind: "action"})
        }}
      />

      <FileInput
        {...fileCopy}
        id="single-max-files-file"
        label="Single maximum file"
        value={null}
        maxFiles={1}
        onChange={(files) => {
          log({field: "single-max-files-file", kind: "change", value: oneFile(files)?.name ?? "null"})
        }}
        onValidationError={(error) => {
          log({field: "single-max-files-file", kind: "error", reason: error.reason})
        }}
        changeAction={() => {
          log({field: "single-max-files-file", kind: "action"})
        }}
      />

      <FileInput
        {...fileCopy}
        id="size-file"
        label="Size limited file"
        value={null}
        maxSize={384 * 1024}
        onChange={(files) => {
          log({field: "size-file", kind: "change", value: oneFile(files)?.name ?? "null"})
        }}
        onValidationError={(error) => {
          log({field: "size-file", kind: "error", reason: error.reason})
        }}
        changeAction={() => {
          log({field: "size-file", kind: "action"})
        }}
      />

      <FileInput
        {...fileCopy}
        id="throw-file"
        label="Throwing file"
        value={null}
        onChange={() => {
          log({field: "throw-file", kind: "change"})
          throw new Error("throw-file onChange")
        }}
      />

      <FileInput
        {...fileCopy}
        id="throw-validation-file"
        label="Throwing validation file"
        value={null}
        accept="text/plain"
        onChange={() => {
          log({field: "throw-validation-file", kind: "change"})
        }}
        onValidationError={(error) => {
          log({field: "throw-validation-file", kind: "error", reason: error.reason})
          throw new Error("throw-validation-file onValidationError")
        }}
      />

      <FileInput
        {...fileCopy}
        id="empty-file"
        label="Empty file"
        value={null}
        onChange={() => {
          log({field: "empty-file", kind: "change"})
        }}
      />

      <output id="field-log" aria-live="polite" />
    </main>
  )
}

createRoot(document.getElementById("root")!).render(<FieldHarness />)
