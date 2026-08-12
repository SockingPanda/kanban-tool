export function joinClassNames(...classNames: readonly (string | false | null | undefined)[]): string | undefined {
  const value = classNames.filter(Boolean).join(" ")
  return value.length > 0 ? value : undefined
}
