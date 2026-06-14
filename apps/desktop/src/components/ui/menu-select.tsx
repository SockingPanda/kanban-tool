import { ChevronDown } from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { cn } from "@/lib/utils"

export type MenuSelectOption<TValue extends string> = {
  value: TValue
  label: string
}

export function MenuSelect<TValue extends string>({
  value,
  options,
  onValueChange,
  ariaLabel,
  prefix,
  className,
  triggerClassName,
  contentClassName,
  align = "start",
}: {
  value: TValue
  options: readonly MenuSelectOption<TValue>[]
  onValueChange: (value: TValue) => void
  ariaLabel: string
  prefix?: string
  className?: string
  triggerClassName?: string
  contentClassName?: string
  align?: "start" | "center" | "end"
}) {
  const selected = options.find((option) => option.value === value)

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          type="button"
          variant="outline"
          size="sm"
          aria-label={ariaLabel}
          className={cn("justify-between", triggerClassName, className)}
        >
          <span className="truncate">
            {prefix ? <span className="text-muted-foreground">{prefix} </span> : null}
            {selected?.label ?? value}
          </span>
          <ChevronDown className="h-3.5 w-3.5 text-muted-foreground" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align={align} className={contentClassName}>
        <DropdownMenuRadioGroup value={value} onValueChange={(next) => onValueChange(next as TValue)}>
          {options.map((option) => (
            <DropdownMenuRadioItem key={option.value} value={option.value}>
              {option.label}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
