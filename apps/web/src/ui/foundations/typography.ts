export const typographyTokens = [
  { key: "body", variable: "--kb-font-size-md", value: "14px" },
  { key: "supporting", variable: "--kb-font-size-sm", value: "13px" },
  { key: "code", variable: "--kb-font-family-code", value: "ui-monospace" },
  { key: "title", variable: "--kb-font-size-xl", value: "20px" },
] as const

export type TypographyToken = (typeof typographyTokens)[number]
