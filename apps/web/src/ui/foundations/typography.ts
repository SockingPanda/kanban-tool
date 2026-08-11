export const typographyTokens = [
  { name: "Body", variable: "--kb-font-size-md", value: "14px", detail: "日常操作与数据阅读" },
  { name: "Supporting", variable: "--kb-font-size-sm", value: "13px", detail: "辅助说明与密集元数据" },
  { name: "Code", variable: "--kb-font-family-code", value: "ui-monospace", detail: "ID、ref、run id 与 hash" },
  { name: "Title", variable: "--kb-font-size-xl", value: "20px", detail: "页面或资源标题" },
] as const

export type TypographyToken = (typeof typographyTokens)[number]
