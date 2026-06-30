import { Settings } from "lucide-react"
import type { ReactNode } from "react"

import { Card } from "@/components/ui/card"
import { Item, ItemActions, ItemContent, ItemTitle } from "@/components/ui/item"
import { MenuSelect, type MenuSelectOption } from "@/components/ui/menu-select"
import { ScrollArea } from "@/components/ui/scroll-area"
import { useI18n, type LocaleMode } from "@/i18n"
import type { RuntimeConfig } from "@/lib/api"

const localeOptions: MenuSelectOption<LocaleMode>[] = [
  { value: "system", label: "System" },
  { value: "zh-CN", label: "中文" },
  { value: "en", label: "English" },
]

export function SettingsView({ config }: { config: RuntimeConfig | null }) {
  const { locale, mode, setMode, t } = useI18n()
  return (
    <ScrollArea className="flex-1 bg-card p-4">
      <Card className="p-4">
        <h2 className="mb-3 flex items-center gap-2 text-sm font-semibold">
          <Settings className="h-4 w-4 text-muted-foreground" />
          {t("Read-only settings")}
        </h2>
        <div className="space-y-3 text-sm">
          <InfoRow label={t("Locale")} value={locale}>
            <MenuSelect
              ariaLabel={t("Locale")}
              value={mode}
              options={localeOptions.map((option) => ({ ...option, label: t(option.label) }))}
              onValueChange={setMode}
              triggerClassName="h-8 min-w-32"
            />
          </InfoRow>
          <InfoRow label="board" value={config?.board ?? "-"} />
          <InfoRow label="actor" value={config?.actor ?? "-"} />
          <InfoRow label="api base" value={config?.apiBaseUrl || "same-origin"} />
          <InfoRow label={t("database")} value={config?.dbPath ?? "-"} />
        </div>
      </Card>
    </ScrollArea>
  )
}

function InfoRow({ children, label, value }: { children?: ReactNode; label: string; value: string }) {
  return (
    <Item className="border-border bg-card px-3 py-2">
      <ItemContent>
        <ItemTitle className="text-muted-foreground">{label}</ItemTitle>
      </ItemContent>
      <ItemActions className="min-w-0">
        {children ?? <span className="truncate font-medium">{value}</span>}
      </ItemActions>
    </Item>
  )
}
