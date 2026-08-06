/* eslint-disable react-refresh/only-export-components */
import * as React from "react"
import { PanelLeft } from "lucide-react"

import { Button, type ButtonProps } from "@/components/ui/button"
import { useI18n } from "@/i18n"
import { cn } from "@/lib/utils"

type SidebarContextValue = {
  open: boolean
  setOpen: (open: boolean) => void
}

const SidebarContext = React.createContext<SidebarContextValue | null>(null)

function SidebarProvider({
  className,
  open: openProp,
  defaultOpen = true,
  onOpenChange,
  children,
  ...props
}: React.ComponentProps<"div"> & {
  open?: boolean
  defaultOpen?: boolean
  onOpenChange?: (open: boolean) => void
}) {
  const [uncontrolledOpen, setUncontrolledOpen] = React.useState(defaultOpen)
  const open = openProp ?? uncontrolledOpen

  const setOpen = React.useCallback(
    (value: boolean) => {
      if (openProp === undefined) setUncontrolledOpen(value)
      onOpenChange?.(value)
    },
    [onOpenChange, openProp],
  )

  const contextValue = React.useMemo(() => ({ open, setOpen }), [open, setOpen])

  return (
    <SidebarContext.Provider value={contextValue}>
      <div data-slot="sidebar-wrapper" className={cn("flex min-h-svh w-full", className)} {...props}>
        {children}
      </div>
    </SidebarContext.Provider>
  )
}

function useSidebar() {
  const context = React.useContext(SidebarContext)
  if (!context) {
    throw new Error("useSidebar 必须在 SidebarProvider 内使用。")
  }
  return context
}

function Sidebar({ className, ...props }: React.ComponentProps<"aside">) {
  return (
    <aside
      data-slot="sidebar"
      className={cn("flex shrink-0 flex-col overflow-hidden border-r border-border bg-sidebar transition-[width] duration-200", className)}
      {...props}
    />
  )
}

function SidebarHeader({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="sidebar-header"
      className={cn("flex h-14 items-center gap-2 px-3 max-sm:justify-center max-sm:px-2", className)}
      {...props}
    />
  )
}

function SidebarContent({ className, ...props }: React.ComponentProps<"nav">) {
  return <nav data-slot="sidebar-content" className={cn("space-y-4 px-2 py-3", className)} {...props} />
}

function SidebarFooter({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="sidebar-footer"
      className={cn("mt-auto space-y-3 border-t border-border p-3 text-xs text-muted-foreground max-sm:hidden", className)}
      {...props}
    />
  )
}

function SidebarGroup({ className, ...props }: React.ComponentProps<"div">) {
  return <div data-slot="sidebar-group" className={className} {...props} />
}

function SidebarGroupLabel({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="sidebar-group-label"
      className={cn("mb-1 px-2 text-[11px] font-medium uppercase tracking-normal text-muted-foreground max-sm:sr-only", className)}
      {...props}
    />
  )
}

function SidebarMenu({ className, ...props }: React.ComponentProps<"ul">) {
  return <ul data-slot="sidebar-menu" className={cn("space-y-1", className)} {...props} />
}

function SidebarMenuItem({ className, ...props }: React.ComponentProps<"li">) {
  return <li data-slot="sidebar-menu-item" className={className} {...props} />
}

function SidebarMenuButton({ className, active = false, ...props }: ButtonProps & { active?: boolean }) {
  return (
    <Button
      data-slot="sidebar-menu-button"
      data-active={active}
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

function SidebarTrigger({ className, onClick, ...props }: React.ComponentProps<typeof Button>) {
  const { open, setOpen } = useSidebar()
  const { t } = useI18n()

  return (
    <Button
      data-slot="sidebar-trigger"
      variant="ghost"
      size="icon"
      className={className}
      onClick={(event) => {
        onClick?.(event)
        setOpen(!open)
      }}
      {...props}
    >
      <PanelLeft className="h-4 w-4" />
      <span className="sr-only">{t("Toggle sidebar")}</span>
    </Button>
  )
}

function SidebarInset({ className, ...props }: React.ComponentProps<"main">) {
  return <main data-slot="sidebar-inset" className={cn("min-w-0 flex-1", className)} {...props} />
}

function SidebarRail({ className, ...props }: React.ComponentProps<typeof Button>) {
  const { t } = useI18n()
  return (
    <Button
      data-slot="sidebar-rail"
      aria-label={t("Toggle sidebar")}
      tabIndex={-1}
      variant="ghost"
      className={cn(
        "absolute inset-y-0 z-20 hidden w-4 -translate-x-1/2 transition-colors hover:bg-sidebar-accent sm:flex",
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
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarRail,
  SidebarTrigger,
  useSidebar,
}
