import * as React from "react"

import { Button, type ButtonProps } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import { cn } from "@/lib/utils"

function InputGroup({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      role="group"
      className={cn(
        "flex min-w-0 items-center rounded-md border border-border bg-input transition-colors focus-within:border-ring focus-within:ring-2 focus-within:ring-ring focus-within:ring-offset-1 has-[textarea]:items-stretch",
        className,
      )}
      {...props}
    />
  )
}

function InputGroupInput({ className, ...props }: React.InputHTMLAttributes<HTMLInputElement>) {
  return (
    <Input
      className={cn("min-w-0 flex-1 rounded-none border-0 bg-transparent focus:border-transparent focus-visible:ring-0 focus-visible:ring-offset-0", className)}
      {...props}
    />
  )
}

function InputGroupTextarea({ className, ...props }: React.TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return (
    <Textarea
      className={cn("min-h-8 min-w-0 flex-1 rounded-none border-0 bg-transparent focus:border-transparent focus-visible:ring-0 focus-visible:ring-offset-0", className)}
      {...props}
    />
  )
}

function InputGroupAddon({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("flex shrink-0 items-center gap-1.5 px-2 text-sm text-muted-foreground", className)} {...props} />
}

function InputGroupButton({ className, variant = "ghost", size = "icon", ...props }: ButtonProps) {
  return <Button variant={variant} size={size} className={cn("shrink-0 rounded-l-none rounded-r-md", className)} {...props} />
}

export { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput, InputGroupTextarea }
