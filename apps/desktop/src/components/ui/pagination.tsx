import * as React from "react"

import { cn } from "@/lib/utils"

function Pagination({ className, ...props }: React.ComponentProps<"nav">) {
  return <nav aria-label="pagination" className={cn("flex items-center", className)} {...props} />
}

function PaginationContent({ className, ...props }: React.ComponentProps<"ul">) {
  return <ul className={cn("flex flex-row items-center gap-1", className)} {...props} />
}

function PaginationItem({ className, ...props }: React.ComponentProps<"li">) {
  return <li className={className} {...props} />
}

export { Pagination, PaginationContent, PaginationItem }
