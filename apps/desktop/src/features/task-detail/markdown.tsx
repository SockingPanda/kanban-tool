import ReactMarkdown, { defaultUrlTransform, type Components, type Options } from "react-markdown"
import remarkGfm from "remark-gfm"

import { cn } from "@/lib/utils"

const MARKDOWN_REMARK_PLUGINS: Options["remarkPlugins"] = [remarkGfm]

const MARKDOWN_COMPONENTS: Components = {
  a: ({ href, children, ...props }) => (
    <a href={href} target="_blank" rel="noreferrer noopener" {...props}>
      {children}
    </a>
  ),
}

export function MarkdownDescription({ children, className }: { children: string; className?: string }) {
  return (
    <div className={cn("task-markdown mt-2 text-sm text-foreground", className)}>
      <ReactMarkdown
        remarkPlugins={MARKDOWN_REMARK_PLUGINS}
        urlTransform={safeMarkdownUrl}
        components={MARKDOWN_COMPONENTS}
      >
        {children}
      </ReactMarkdown>
    </div>
  )
}

function safeMarkdownUrl(value: string) {
  return defaultUrlTransform(value)
}

export const __test = {
  markdownRemarkPlugins: MARKDOWN_REMARK_PLUGINS,
  markdownComponents: MARKDOWN_COMPONENTS,
}
