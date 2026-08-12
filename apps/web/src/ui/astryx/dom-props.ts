import type {
  AriaAttributes,
  AriaRole,
  FocusEventHandler,
  InputEventHandler,
  KeyboardEventHandler,
  MouseEventHandler,
  ReactEventHandler,
} from "react"

/**
 * Runtime allowlist for props forwarded to native DOM elements.
 *
 * The TypeScript surface of a component can prevent most accidental spreads,
 * but JavaScript callers and `...rest` objects still cross this boundary at
 * runtime. Keep this list deliberately small: semantic `aria-*`/`data-*`
 * attributes, a finite set of native attributes, and event handlers whose
 * values are functions. Presentation hooks and unknown props are discarded.
 */

const SAFE_ATTRIBUTE_NAMES = new Set([
  "dir",
  "draggable",
  "hidden",
  "inert",
  "lang",
  "role",
  "tabIndex",
  "title",
  "translate",
])

const SAFE_HANDLER_NAMES = new Set([
  "onFocus",
  "onBlur",
  "onKeyDown",
  "onKeyUp",
  "onKeyPress",
  "onClick",
  "onMouseDown",
  "onInput",
  "onInvalid",
])

/**
 * The public prop surface accepted by {@link pickCspSafeDomProps}.
 *
 * This intentionally does not extend React's broad `HTMLAttributes`: every
 * key here is either copied by the runtime allowlist or is an ARIA/data
 * semantic attribute. Component-owned `id`, `className`, `children`, and
 * `ref` values remain explicit at their element sites.
 */
export type CspSafeDomProps<E extends Element = HTMLElement> = AriaAttributes & {
  readonly [name: `data-${string}`]: string | number | boolean | undefined
  readonly dir?: string
  readonly draggable?: boolean | "true" | "false"
  readonly hidden?: boolean
  readonly inert?: boolean
  readonly lang?: string
  readonly role?: AriaRole
  readonly tabIndex?: number
  readonly title?: string
  readonly translate?: "yes" | "no"
  readonly onFocus?: FocusEventHandler<E>
  readonly onBlur?: FocusEventHandler<E>
  readonly onKeyDown?: KeyboardEventHandler<E>
  readonly onKeyUp?: KeyboardEventHandler<E>
  readonly onKeyPress?: KeyboardEventHandler<E>
  readonly onClick?: MouseEventHandler<E>
  readonly onMouseDown?: MouseEventHandler<E>
  readonly onInput?: InputEventHandler<E>
  readonly onInvalid?: ReactEventHandler<E>
}

function isSafeHandlerName(name: string): boolean {
  return SAFE_HANDLER_NAMES.has(name)
}

/**
 * Pick the CSP-safe subset of a caller-provided DOM prop bag.
 *
 * `className`, `children`, `ref`, `style`, `xstyle`,
 * `dangerouslySetInnerHTML`, and every unknown key are intentionally omitted;
 * component-owned values must be supplied explicitly at the element site.
 * Input objects are never mutated.
 */
export function pickCspSafeDomProps<E extends Element = HTMLElement>(props: object | null | undefined): CspSafeDomProps<E> {
  if (props === null || props === undefined) return {}

  const safe: Record<string, unknown> = {}
  for (const [name, value] of Object.entries(props)) {
    if (name.startsWith("aria-") || name.startsWith("data-") || SAFE_ATTRIBUTE_NAMES.has(name)) {
      safe[name] = value
      continue
    }
    if (isSafeHandlerName(name) && typeof value === "function") {
      safe[name] = value
    }
  }
  return safe as CspSafeDomProps<E>
}
