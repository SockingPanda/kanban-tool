import {
  forwardRef,
  type ReactNode,
  type Ref,
} from "react"

import {
  DropdownMenu,
  type DropdownMenuOption,
} from "./DropdownMenu"

export interface MoreMenuProps {
  readonly items: readonly DropdownMenuOption[]
  readonly label?: string
  readonly variant?: "primary" | "secondary" | "ghost"
  readonly size?: "sm" | "md" | "lg"
  readonly icon?: ReactNode
  readonly isDisabled?: boolean
  readonly isMenuOpen?: boolean
  readonly onOpenChange?: (isOpen: boolean) => void
  readonly className?: string
  readonly "data-testid"?: string
}

function MoreDots(): ReactNode {
  return "⋯"
}

export const MoreMenu = forwardRef<HTMLButtonElement, MoreMenuProps>(function MoreMenu(
  {
    items,
    label = "More options",
    variant = "ghost",
    size = "md",
    icon,
    isDisabled = false,
    isMenuOpen,
    onOpenChange,
    className,
    "data-testid": testId,
  },
  ref: Ref<HTMLButtonElement>,
) {
  return (
    <DropdownMenu
      button={{
        label,
        icon: icon ?? <MoreDots />,
        variant,
        size,
        isDisabled,
        isIconOnly: true,
        className,
        "aria-label": label,
      }}
      items={items}
      isMenuOpen={isMenuOpen}
      onOpenChange={onOpenChange}
      hasChevron={false}
      triggerRef={ref}
      data-testid={testId}
    />
  )
})

MoreMenu.displayName = "AstryxMoreMenu"

export const AstryxMoreMenu = MoreMenu
