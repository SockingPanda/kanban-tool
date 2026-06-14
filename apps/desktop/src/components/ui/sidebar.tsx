import * as React from "react"

import { Button, type ButtonProps } from "@/components/ui/button"
import { cn } from "@/lib/utils"

function Sidebar({ className, ...props }: React.ComponentProps<"aside">) {
  return (
    <aside
      className={cn("flex shrink-0 flex-col overflow-hidden border-r border-border bg-sidebar transition-[width] duration-200", className)}
      {...props}
    />
  )
}

function SidebarHeader({ className, ...props }: React.ComponentProps<"div">) {
  return <div className={cn("flex h-14 items-center gap-2 px-3 max-sm:justify-center max-sm:px-2", className)} {...props} />
}

function SidebarContent({ className, ...props }: React.ComponentProps<"nav">) {
  return <nav className={cn("space-y-4 px-2 py-3", className)} {...props} />
}

function SidebarFooter({ className, ...props }: React.ComponentProps<"div">) {
  return <div className={cn("mt-auto space-y-3 border-t border-border p-3 text-xs text-muted-foreground max-sm:hidden", className)} {...props} />
}

function SidebarGroup({ className, ...props }: React.ComponentProps<"div">) {
  return <div className={className} {...props} />
}

function SidebarGroupLabel({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      className={cn("mb-1 px-2 text-[11px] font-medium uppercase tracking-normal text-muted-foreground max-sm:sr-only", className)}
      {...props}
    />
  )
}

function SidebarMenu({ className, ...props }: React.ComponentProps<"div">) {
  return <div className={cn("space-y-1", className)} {...props} />
}

function SidebarMenuButton({ className, active = false, ...props }: ButtonProps & { active?: boolean }) {
  return (
    <Button
      type="button"
      variant="ghost"
      className={cn(
        "h-auto w-full justify-start gap-2 px-2 py-1.5 text-sm text-muted-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground",
        "max-sm:justify-center",
        active && "bg-sidebar-accent text-sidebar-accent-foreground",
        className,
      )}
      {...props}
    />
  )
}

export {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
}
