import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

import { getCurrentDesktopLocale, type DesktopLocale } from "@/i18n"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export function formatRelativeTime(value?: number | null, locale: DesktopLocale = getCurrentDesktopLocale()) {
  if (!value) return locale === "zh-CN" ? "无" : "none"
  const delta = Date.now() - value
  const abs = Math.abs(delta)
  const amount =
    abs < 60_000
      ? Math.max(1, Math.round(abs / 1000))
      : abs < 3_600_000
        ? Math.round(abs / 60_000)
        : abs < 86_400_000
          ? Math.round(abs / 3_600_000)
          : Math.round(abs / 86_400_000)
  const unit = abs < 60_000 ? "s" : abs < 3_600_000 ? "m" : abs < 86_400_000 ? "h" : "d"
  if (locale === "zh-CN") {
    const zhUnit = unit === "s" ? "秒" : unit === "m" ? "分钟" : unit === "h" ? "小时" : "天"
    return `${amount}${zhUnit}${delta >= 0 ? "前" : "后"}`
  }
  return `${amount}${unit} ${delta >= 0 ? "ago" : "from now"}`
}

export function shortId(id?: string | null) {
  if (!id) return "-"
  return id.length <= 12 ? id : `${id.slice(0, 6)}...${id.slice(-4)}`
}
