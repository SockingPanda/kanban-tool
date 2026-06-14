import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
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
    <Select value={value} onValueChange={(next) => onValueChange(next as TValue)}>
      <SelectTrigger aria-label={ariaLabel} className={cn("justify-between", triggerClassName, className)}>
        <SelectValue aria-label={selected?.label ?? value}>
          {prefix ? <span className="text-muted-foreground">{prefix} </span> : null}
          {selected?.label ?? value}
        </SelectValue>
      </SelectTrigger>
      <SelectContent align={align} className={contentClassName}>
        {options.map((option) => (
          <SelectItem key={option.value} value={option.value}>
            {option.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}
