import { Settings } from "lucide-react"

import type { RuntimeConfig } from "@/lib/api"

export function SettingsView({ config }: { config: RuntimeConfig | null }) {
  return (
    <div className="min-h-0 flex-1 overflow-auto bg-white p-4">
      <section className="rounded-md border border-neutral-200 bg-neutral-50 p-4">
        <h2 className="mb-3 flex items-center gap-2 text-sm font-semibold">
          <Settings className="h-4 w-4 text-neutral-500" />
          Read-only settings
        </h2>
        <div className="space-y-2 text-sm">
          <InfoRow label="board" value={config?.board ?? "-"} />
          <InfoRow label="actor" value={config?.actor ?? "-"} />
          <InfoRow label="api base" value={config?.apiBaseUrl || "same-origin"} />
          <InfoRow label="database" value={config?.dbPath ?? "-"} />
        </div>
      </section>
    </div>
  )
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between gap-3 rounded border border-neutral-200 bg-white px-3 py-2">
      <span className="text-neutral-500">{label}</span>
      <span className="truncate font-medium">{value}</span>
    </div>
  )
}
