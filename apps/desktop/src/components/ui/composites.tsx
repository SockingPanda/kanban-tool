import type { ElementType, ReactNode } from "react"

import { Badge, type BadgeProps } from "@/components/ui/badge"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Item, ItemActions, ItemContent, ItemDescription, ItemTitle } from "@/components/ui/item"
import type { TaskStatus } from "@/lib/api"
import { priorityBadgeClass, priorityLabel } from "@/lib/priority"
import { cn, shortId } from "@/lib/utils"

export function PageToolbar({
  title,
  description,
  leading,
  children,
  meta,
  className,
}: {
  title?: ReactNode
  description?: ReactNode
  leading?: ReactNode
  children?: ReactNode
  meta?: ReactNode
  className?: string
}) {
  return (
    <div className={cn("flex min-h-10 flex-wrap items-center gap-2 border-b border-border px-4 py-3 text-sm", className)}>
      {leading}
      {title || description ? (
        <div className="min-w-0">
          {title ? <div className="truncate font-medium">{title}</div> : null}
          {description ? <div className="truncate text-xs text-muted-foreground">{description}</div> : null}
        </div>
      ) : null}
      {children}
      {meta ? <div className="ml-auto flex min-w-0 items-center gap-2 text-xs text-muted-foreground">{meta}</div> : null}
    </div>
  )
}

export function SectionCard({
  title,
  icon: Icon,
  actions,
  children,
  className,
}: {
  title: ReactNode
  icon?: ElementType
  actions?: ReactNode
  children: ReactNode
  className?: string
}) {
  return (
    <Card className={className}>
      <CardHeader className="flex-row items-center justify-between gap-3">
        <CardTitle className="flex min-w-0 items-center gap-2">
          {Icon ? <Icon className="h-4 w-4 shrink-0 text-muted-foreground" /> : null}
          <span className="truncate">{title}</span>
        </CardTitle>
        {actions ? <div className="shrink-0">{actions}</div> : null}
      </CardHeader>
      <CardContent>{children}</CardContent>
    </Card>
  )
}

export type MetricStripItem = {
  label: ReactNode
  value: ReactNode
  tone?: BadgeProps["variant"]
}

export function MetricStrip({
  items,
  className,
  itemClassName,
}: {
  items: MetricStripItem[]
  className?: string
  itemClassName?: string
}) {
  return (
    <div className={cn("grid gap-2", className)}>
      {items.map((item, index) => (
        <MetricTile key={typeof item.label === "string" ? item.label : index} item={item} className={itemClassName} />
      ))}
    </div>
  )
}

function MetricTile({ item, className }: { item: MetricStripItem; className?: string }) {
  return (
    <Item className={cn("min-w-0 border-border bg-card p-2", className)}>
      <ItemContent>
        <ItemDescription>{item.label}</ItemDescription>
        <ItemTitle className="mt-1">
          {item.tone ? <Badge variant={item.tone} className="max-w-full truncate">{item.value}</Badge> : item.value}
        </ItemTitle>
      </ItemContent>
    </Item>
  )
}

export function TaskStatusBadge({
  status,
  className,
}: {
  status: TaskStatus | string
  className?: string
}) {
  return (
    <Badge variant={taskStatusBadgeVariant(status)} className={className}>
      {status}
    </Badge>
  )
}

export function taskStatusBadgeVariant(status: TaskStatus | string): BadgeProps["variant"] {
  if (status === "ready" || status === "done") return "ready"
  if (status === "running") return "running"
  if (status === "blocked") return "blocked"
  if (status === "review") return "review"
  return "secondary"
}

export function PriorityBadge({ priority, className }: { priority: number; className?: string }) {
  return (
    <Badge variant="secondary" className={cn(priorityBadgeClass(priority), className)}>
      {priorityLabel(priority)}
    </Badge>
  )
}

export function TaskIdentityLine({
  id,
  ref,
  seq,
  title,
  className,
  titleClassName,
}: {
  id: string
  ref?: string | null
  seq?: number
  title?: ReactNode
  className?: string
  titleClassName?: string
}) {
  const reference = ref || (typeof seq === "number" ? `#${seq}` : shortId(id))
  return (
    <div className={cn("min-w-0", className)}>
      <div className="truncate text-xs text-muted-foreground">
        {reference} · {shortId(id)}
      </div>
      {title ? <div className={cn("break-words text-sm font-semibold", titleClassName)}>{title}</div> : null}
    </div>
  )
}
