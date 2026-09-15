/** Canonical board identity after the runtime selector has been resolved. */
export type CanonicalBoardSlug = string & { readonly __canonicalBoardSlug: unique symbol }

export type CanonicalBoardSlugErrorReason =
  | "missing"
  | "too-long"
  | "invalid-first-character"
  | "invalid-character"
  | "reserved-prefix"

export class CanonicalBoardSlugError extends Error {
  readonly kind = "invalid-canonical-board-slug"
  readonly value: unknown
  readonly reason: CanonicalBoardSlugErrorReason

  constructor(value: unknown, reason: CanonicalBoardSlugErrorReason) {
    super(`Invalid canonical board slug (${reason})`)
    this.name = "CanonicalBoardSlugError"
    this.value = value
    this.reason = reason
  }
}

const RESERVED_PREFIXES = ["b_", "t_", "r_", "c_", "a_", "l_", "col_", "e_"] as const
const CANONICAL_SLUG_PATTERN = /^[a-z0-9][a-z0-9._-]*$/

export function validateCanonicalBoardSlug(value: unknown): CanonicalBoardSlugError | null {
  if (typeof value !== "string" || value.length === 0) return new CanonicalBoardSlugError(value, "missing")
  if (value.length > 64) return new CanonicalBoardSlugError(value, "too-long")
  if (!/^[a-z0-9]/.test(value)) return new CanonicalBoardSlugError(value, "invalid-first-character")
  if (!CANONICAL_SLUG_PATTERN.test(value)) return new CanonicalBoardSlugError(value, "invalid-character")
  if (RESERVED_PREFIXES.some((prefix) => value.startsWith(prefix))) {
    return new CanonicalBoardSlugError(value, "reserved-prefix")
  }
  return null
}

/** Returns a branded slug only for values accepted by the service identity rules. */
export function parseCanonicalBoardSlug(value: unknown): CanonicalBoardSlug | null {
  return validateCanonicalBoardSlug(value) ? null : (value as CanonicalBoardSlug)
}

/** Throws a typed error at an integration boundary that requires canonical identity. */
export function assertCanonicalBoardSlug(value: unknown): CanonicalBoardSlug {
  const error = validateCanonicalBoardSlug(value)
  if (error) throw error
  return value as CanonicalBoardSlug
}
