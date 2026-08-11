import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, it } from "vitest"

import { Button } from "./Button"
import { StateBoundary } from "./StateBoundary"
import { TextField } from "./TextField"

describe("presentational primitives", () => {
  it("keeps loading actions disabled and announced", () => {
    const markup = renderToStaticMarkup(<Button isLoading>Save</Button>)

    expect(markup).toContain("disabled")
    expect(markup).toContain('aria-busy="true"')
  })

  it("connects field errors to the input", () => {
    const markup = renderToStaticMarkup(<TextField label="Project" value="unknown" readOnly error="Project cannot be resolved" />)

    expect(markup).toContain('aria-invalid="true"')
    expect(markup).toContain('role="alert"')
    expect(markup).toContain("Project cannot be resolved")
  })

  it("uses an alert role for recoverable read errors", () => {
    const markup = renderToStaticMarkup(<StateBoundary mode="error" title="Read failed" onRetry={() => undefined} retryLabel="Retry" />)

    expect(markup).toContain('role="alert"')
    expect(markup).toContain("Retry")
  })
})
