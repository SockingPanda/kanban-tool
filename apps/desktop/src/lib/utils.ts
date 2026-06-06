import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export function formatRelativeTime(value?: number | null) {
  if (!value) return "none"
  const delta = Date.now() - value
  const abs = Math.abs(delta)
  const suffix = delta >= 0 ? "ago" : "from now"
  if (abs < 60_000) return `${Math.max(1, Math.round(abs / 1000))}s ${suffix}`
  if (abs < 3_600_000) return `${Math.round(abs / 60_000)}m ${suffix}`
  if (abs < 86_400_000) return `${Math.round(abs / 3_600_000)}h ${suffix}`
  return `${Math.round(abs / 86_400_000)}d ${suffix}`
}

export function shortId(id?: string | null) {
  if (!id) return "-"
  return id.length <= 12 ? id : `${id.slice(0, 6)}...${id.slice(-4)}`
}
