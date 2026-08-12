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

export type CspSafeDomProps = Record<string, unknown>

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
export function pickCspSafeDomProps(props: object | null | undefined): CspSafeDomProps {
  if (props === null || props === undefined) return {}

  const safe: CspSafeDomProps = {}
  for (const [name, value] of Object.entries(props)) {
    if (name.startsWith("aria-") || name.startsWith("data-") || SAFE_ATTRIBUTE_NAMES.has(name)) {
      safe[name] = value
      continue
    }
    if (isSafeHandlerName(name) && typeof value === "function") {
      safe[name] = value
    }
  }
  return safe
}
