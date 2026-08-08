export interface SseFrame {
  readonly eventName: string
  readonly id: string | null
  readonly data: string
}

export interface SseParserOptions {
  /** Maximum encoded bytes for one dispatched frame. */
  readonly maxFrameBytes?: number
  /** Maximum encoded bytes retained while waiting for a line terminator. */
  readonly maxBufferBytes?: number
  /** Maximum number of `data:` fields in one frame. */
  readonly maxDataLines?: number
}

const DEFAULT_MAX_FRAME_BYTES = 256 * 1024
const DEFAULT_MAX_BUFFER_BYTES = 512 * 1024
const DEFAULT_MAX_DATA_LINES = 256

export class SseParseError extends Error {
  readonly code:
    | "buffer_limit"
    | "frame_limit"
    | "data_line_limit"
    | "unsupported_field"
    | "duplicate_field"
    | "missing_event"
    | "missing_data"
    | "incomplete_frame"

  constructor(code: SseParseError["code"], message: string) {
    super(message)
    this.name = "SseParseError"
    this.code = code
  }
}

interface PendingFrame {
  eventName: string | null
  id: string | null
  dataLines: string[]
  sawData: boolean
  sawEvent: boolean
  sawId: boolean
  bytes: number
}

function byteLength(value: string): number {
  return new TextEncoder().encode(value).byteLength
}

function newPendingFrame(): PendingFrame {
  return {
    eventName: null,
    id: null,
    dataLines: [],
    sawData: false,
    sawEvent: false,
    sawId: false,
    bytes: 0,
  }
}

/**
 * Strict, bounded parser for the subset of SSE used by the Web sync path.
 *
 * It deliberately emits every named event (including names unknown to the
 * generated business union) so the contract adapter can route and validate
 * control frames without relying on `EventSource`'s dispatch rules.
 */
export interface SseParser {
  push(chunk: string): readonly SseFrame[]
  finish(): readonly SseFrame[]
}

export function createSseParser(options: SseParserOptions = {}): SseParser {
  const maxFrameBytes = options.maxFrameBytes ?? DEFAULT_MAX_FRAME_BYTES
  const maxBufferBytes = options.maxBufferBytes ?? DEFAULT_MAX_BUFFER_BYTES
  const maxDataLines = options.maxDataLines ?? DEFAULT_MAX_DATA_LINES

  if (!Number.isSafeInteger(maxFrameBytes) || maxFrameBytes <= 0) {
    throw new RangeError("maxFrameBytes must be a positive safe integer")
  }
  if (!Number.isSafeInteger(maxBufferBytes) || maxBufferBytes <= 0) {
    throw new RangeError("maxBufferBytes must be a positive safe integer")
  }
  if (!Number.isSafeInteger(maxDataLines) || maxDataLines <= 0) {
    throw new RangeError("maxDataLines must be a positive safe integer")
  }

  let lineBuffer = ""
  let lineBufferBytes = 0
  let pending = newPendingFrame()
  let ended = false

  function fail(error: SseParseError): never {
    ended = true
    throw error
  }

  function checkFrameBytes(nextBytes: number): void {
    if (nextBytes > maxFrameBytes) {
      fail(new SseParseError("frame_limit", `SSE frame exceeds ${maxFrameBytes} bytes`))
    }
  }

  function dispatch(): SseFrame | null {
    if (pending.eventName === null) {
      if (!pending.sawData && !pending.sawId) {
        pending = newPendingFrame()
        return null
      }
      fail(new SseParseError("missing_event", "SSE frame is missing event"))
    }
    if (!pending.sawData) {
      pending = newPendingFrame()
      return null
    }

    const frame: SseFrame = {
      eventName: pending.eventName,
      id: pending.id,
      data: pending.dataLines.join("\n"),
    }
    pending = newPendingFrame()
    return frame
  }

  function consumeLine(rawLine: string): SseFrame | null {
    const line = rawLine.endsWith("\r") ? rawLine.slice(0, -1) : rawLine
    pending.bytes += byteLength(rawLine) + 1
    checkFrameBytes(pending.bytes)
    if (line === "") return dispatch()
    if (line.startsWith(":")) return null

    const separator = line.indexOf(":")
    const field = separator === -1 ? line : line.slice(0, separator)
    let value = separator === -1 ? "" : line.slice(separator + 1)
    if (value.startsWith(" ")) value = value.slice(1)

    if (field === "event") {
      if (pending.sawEvent) fail(new SseParseError("duplicate_field", "SSE frame repeats event"))
      if (value === "") fail(new SseParseError("missing_event", "SSE event name is empty"))
      pending.eventName = value
      pending.sawEvent = true
      return null
    }
    if (field === "id") {
      if (pending.sawId) fail(new SseParseError("duplicate_field", "SSE frame repeats id"))
      pending.id = value
      pending.sawId = true
      return null
    }
    if (field === "data") {
      if (pending.dataLines.length >= maxDataLines) {
        fail(new SseParseError("data_line_limit", `SSE frame exceeds ${maxDataLines} data lines`))
      }
      pending.dataLines.push(value)
      pending.sawData = true
      return null
    }
    fail(new SseParseError("unsupported_field", `SSE frame has unsupported field: ${field}`))
  }

  return {
    push(chunk: string): readonly SseFrame[] {
      if (ended) throw new SseParseError("incomplete_frame", "SSE parser is already finished")
      // A transport chunk may contain many complete frames.  Retained-buffer
      // limits apply only after those lines have been consumed; checking the
      // whole chunk first would reject a legal burst merely because it is
      // larger than the incomplete-line budget.
      lineBuffer += chunk
      lineBufferBytes += byteLength(chunk)
      const frames: SseFrame[] = []
      let newline = lineBuffer.indexOf("\n")
      while (newline !== -1) {
        const line = lineBuffer.slice(0, newline)
        lineBuffer = lineBuffer.slice(newline + 1)
        lineBufferBytes = byteLength(lineBuffer)
        const frame = consumeLine(line)
        if (frame !== null) frames.push(frame)
        newline = lineBuffer.indexOf("\n")
      }
      if (lineBufferBytes > maxBufferBytes) {
        fail(new SseParseError("buffer_limit", `SSE buffer exceeds ${maxBufferBytes} bytes`))
      }
      checkFrameBytes(pending.bytes + lineBufferBytes)
      return frames
    },

    finish(): readonly SseFrame[] {
      if (ended) throw new SseParseError("incomplete_frame", "SSE parser is already finished")
      if (lineBuffer !== "" || pending.sawEvent || pending.sawData || pending.sawId) {
        fail(new SseParseError("incomplete_frame", "incomplete SSE frame at EOF"))
      }
      ended = true
      return []
    },
  }
}
