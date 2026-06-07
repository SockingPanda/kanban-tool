import { describe, expect, it, vi } from "vitest"

import { createLatestRequestGuard, runLatestRequest } from "./latest-request"

describe("runLatestRequest", () => {
  it("keeps stale requests from committing when the latest request resolves first", async () => {
    const guard = createLatestRequestGuard()
    const commits: string[] = []
    const first = deferred<string>()
    const second = deferred<string>()

    const firstRun = runLatestRequest(guard, () => first.promise, (result) => commits.push(result))
    const secondRun = runLatestRequest(guard, () => second.promise, (result) => commits.push(result))

    second.resolve("fresh")
    await expect(secondRun).resolves.toBe(true)
    first.resolve("stale")
    await expect(firstRun).resolves.toBe(false)

    expect(commits).toEqual(["fresh"])
  })

  it("lets the latest request replace an older request that resolves first", async () => {
    const guard = createLatestRequestGuard()
    const commits: string[] = []
    const first = deferred<string>()
    const second = deferred<string>()

    const firstRun = runLatestRequest(guard, () => first.promise, (result) => commits.push(result))
    const secondRun = runLatestRequest(guard, () => second.promise, (result) => commits.push(result))

    first.resolve("stale")
    await expect(firstRun).resolves.toBe(false)
    second.resolve("fresh")
    await expect(secondRun).resolves.toBe(true)

    expect(commits).toEqual(["fresh"])
  })

  it("does not surface errors from stale requests", async () => {
    const guard = createLatestRequestGuard()
    const onLatestError = vi.fn()
    const first = deferred<string>()
    const second = deferred<string>()

    const firstRun = runLatestRequest(
      guard,
      () => first.promise,
      () => undefined,
      onLatestError,
    )
    const secondRun = runLatestRequest(
      guard,
      () => second.promise,
      () => undefined,
      onLatestError,
    )

    first.reject(new Error("stale failure"))
    await expect(firstRun).resolves.toBe(false)
    second.resolve("fresh")
    await expect(secondRun).resolves.toBe(true)

    expect(onLatestError).not.toHaveBeenCalled()
  })
})

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (error: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}
