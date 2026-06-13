export const SIDEBAR_OPEN_STORAGE_KEY = "kb:desktop:sidebar-open"

export function parseSidebarOpen(value: unknown, fallback = true) {
  if (value === "true") return true
  if (value === "false") return false
  return fallback
}

export function serializeSidebarOpen(open: boolean) {
  return String(open)
}
