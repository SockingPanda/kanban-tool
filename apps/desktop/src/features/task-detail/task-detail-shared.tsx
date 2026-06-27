import { useLayoutEffect, useRef, type ChangeEvent, type ReactNode } from "react"

import { Item, ItemActions, ItemContent, ItemTitle } from "@/components/ui/item"
import { Textarea } from "@/components/ui/textarea"
import { type MenuSelectOption } from "@/components/ui/menu-select"
import { priorityLabel, priorityLevels } from "@/lib/priority"

export const priorityOptions: MenuSelectOption<string>[] = priorityLevels.map((priority) => ({
  value: String(priority),
  label: priorityLabel(priority),
}))

export function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h3 className="mb-2 text-xs font-semibold uppercase tracking-normal text-muted-foreground">{title}</h3>
      {children}
    </section>
  )
}

export function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <Item className="px-0 py-0">
      <ItemContent>
        <ItemTitle className="text-sm font-normal text-muted-foreground">{label}</ItemTitle>
      </ItemContent>
      <ItemActions className="min-w-0">
        <span className="truncate font-medium">{value}</span>
      </ItemActions>
    </Item>
  )
}

export function AutosizeDescriptionTextarea({
  value,
  onChange,
  placeholder,
}: {
  value: string
  onChange: (value: string) => void
  placeholder: string
}) {
  const textareaRef = useRef<HTMLTextAreaElement | null>(null)

  useLayoutEffect(() => {
    autosizeTextarea(textareaRef.current)
  }, [value])

  function handleChange(event: ChangeEvent<HTMLTextAreaElement>) {
    onChange(event.target.value)
    autosizeTextarea(event.currentTarget)
  }

  return (
    <Textarea
      ref={textareaRef}
      className="min-h-28 overflow-y-hidden"
      aria-label={placeholder}
      name="task-description"
      autoComplete="off"
      value={value}
      onChange={handleChange}
      placeholder={placeholder}
    />
  )
}

function autosizeTextarea(textarea: HTMLTextAreaElement | null) {
  if (!textarea) return
  textarea.style.height = "auto"
  textarea.style.height = `${textarea.scrollHeight}px`
}
