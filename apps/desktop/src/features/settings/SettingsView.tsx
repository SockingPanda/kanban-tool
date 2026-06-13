import { Settings } from "lucide-react"

import { Card } from "@/components/ui/card"
import { ScrollArea } from "@/components/ui/scroll-area"
import type { RuntimeConfig } from "@/lib/api"

export function SettingsView({ config }: { config: RuntimeConfig | null }) {
  return (
    <ScrollArea className="flex-1 bg-card p-4">
      <Card className="p-4">
        <h2 className="mb-3 flex items-center gap-2 text-sm font-semibold">
          <Settings className="h-4 w-4 text-muted-foreground" />
          Read-only settings
        </h2>
        <div className="space-y-2 text-sm">
          <InfoRow label="board" value={config?.board ?? "-"} />
          <InfoRow label="actor" value={config?.actor ?? "-"} />
          <InfoRow label="api base" value={config?.apiBaseUrl || "same-origin"} />
          <InfoRow label="database" value={config?.dbPath ?? "-"} />
        </div>
      </Card>
    </ScrollArea>
  )
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <Card className="flex justify-between gap-3 px-3 py-2">
      <span className="text-muted-foreground">{label}</span>
      <span className="truncate font-medium">{value}</span>
    </Card>
  )
}
