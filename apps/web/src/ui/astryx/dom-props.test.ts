import {describe, expect, test, vi} from "vitest"

import {pickCspSafeDomProps, type CspSafeDomProps} from "./dom-props"

const typedSafeProps: CspSafeDomProps<HTMLInputElement> = {
  "aria-label": "safe",
  "data-state": "ready",
  dir: "ltr",
  onInput: () => undefined,
  onInvalid: () => undefined,
}

// @ts-expect-error Presentation props are not part of the finite safe surface.
const typedStyleProps: CspSafeDomProps<HTMLInputElement> = {style: {color: "red"}}
// @ts-expect-error Handlers outside the runtime allowlist are not public props.
const typedUnsafeHandlerProps: CspSafeDomProps<HTMLInputElement> = {onPaste: () => undefined}

void typedSafeProps
void typedStyleProps
void typedUnsafeHandlerProps

describe("pickCspSafeDomProps", () => {
  test("keeps semantic attributes and finite function handlers", () => {
    const onClick = vi.fn()
    const source = {
      "aria-label": "safe",
      "data-state": "ready",
      dir: "ltr",
      draggable: false,
      hidden: true,
      inert: false,
      lang: "zh-CN",
      role: "button",
      tabIndex: 0,
      title: "title",
      translate: "no",
      onClick,
      onInput: () => undefined,
    }

    expect(pickCspSafeDomProps(source)).toEqual(source)
  })

  test("drops presentation, ownership, unknown, and non-function handler props", () => {
    const source = {
      className: "unsafe-class",
      children: "unsafe-children",
      ref: () => undefined,
      style: {color: "red"},
      xstyle: {color: "red"},
      dangerouslySetInnerHTML: {__html: "<style>"},
      onClick: "not-a-function",
      onPaste: () => undefined,
      customProp: "unknown",
    }

    expect(pickCspSafeDomProps(source)).toEqual({})
    expect(source.style).toEqual({color: "red"})
  })

  test("accepts nullish input without throwing", () => {
    expect(pickCspSafeDomProps(undefined)).toEqual({})
    expect(pickCspSafeDomProps(null)).toEqual({})
  })
})
