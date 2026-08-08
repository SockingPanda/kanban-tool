import { describe, expect, test } from "vitest"

import { SseParseError, createSseParser, type SseFrame } from "./sse-parser"

describe("bounded SSE parser", () => {
  test("parses frames split across arbitrary chunks and preserves unknown event names", () => {
    const parser = createSseParser()

    expect(parser.push("event: future.event\nid: 41\nda")).toEqual([])
    expect(parser.push("ta: {\"id\":41}\n\n")).toEqual([
      {
        eventName: "future.event",
        id: "41",
        data: '{"id":41}',
      },
    ] satisfies readonly SseFrame[])
    expect(parser.finish()).toEqual([])
  })

  test("supports CRLF and joins multiple data lines with LF", () => {
    const parser = createSseParser()

    expect(parser.push("event: message\r\nid: 7\r\ndata: first\r\ndata: second\r\n\r\n")).toEqual([
      {
        eventName: "message",
        id: "7",
        data: "first\nsecond",
      },
    ])
  })

  test("rejects duplicate fields and unsupported fields", () => {
    const duplicate = createSseParser()
    expect(() => duplicate.push("event: a\nevent: b\n\n")).toThrowError(SseParseError)

    const unsupported = createSseParser()
    expect(() => unsupported.push("retry: 1000\n\n")).toThrowError(SseParseError)
  })

  test("rejects incomplete frames at EOF", () => {
    const parser = createSseParser()
    parser.push("event: message\nid: 1\ndata: {}\n")

    expect(() => parser.finish()).toThrowError(/incomplete SSE frame/)
  })

  test("enforces frame, data-line, and pending-buffer bounds", () => {
    expect(() => createSseParser({ maxFrameBytes: 8 }).push("event: too-long\n")).toThrowError(/frame exceeds/)
    expect(() => createSseParser({ maxDataLines: 1 }).push("data: a\ndata: b\n\n")).toThrowError(/data lines/)
    expect(() => createSseParser({ maxBufferBytes: 4 }).push("event:")).toThrowError(/buffer exceeds/)
  })

  test("consumes a large chunk of complete frames before applying retained-buffer limits", () => {
    const parser = createSseParser({ maxBufferBytes: 16, maxFrameBytes: 128 })
    const chunk = Array.from({ length: 2_000 }, (_, index) => `event: e\nid: ${index}\ndata: {}\n\n`).join("")
    expect(parser.push(chunk)).toHaveLength(2_000)
  })

  test("dispatches named frames without data so the adapter can fail closed", () => {
    const parser = createSseParser()

    expect(parser.push(": keepalive\n\n")).toEqual([])
    expect(parser.push("event: empty\n\n")).toEqual([
      { eventName: "empty", id: null, data: "" },
    ])
    expect(parser.push("event: data\ndata:\n\n")).toEqual([
      { eventName: "data", id: null, data: "" },
    ])
  })

  test("preserves a data-less named frame across chunk boundaries", () => {
    const parser = createSseParser()

    expect(parser.push("event: kb-heartbeat\n")).toEqual([])
    expect(parser.push("\n")).toEqual([
      { eventName: "kb-heartbeat", id: null, data: "" },
    ])
  })
})
