import { keepPreviousData, useQuery } from "@tanstack/react-query"
import { Settings } from "lucide-react"
import type { ReactNode } from "react"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Card } from "@/components/ui/card"
import { Item, ItemActions, ItemContent, ItemTitle } from "@/components/ui/item"
import { MenuSelect, type MenuSelectOption } from "@/components/ui/menu-select"
import { ScrollArea } from "@/components/ui/scroll-area"
import { useI18n, type LocaleMode } from "@/i18n"
import type { KanbanApi, RuntimeConfig } from "@/lib/api"
import { presentApiError } from "@/lib/api/error-presentation"

const localeOptions: MenuSelectOption<LocaleMode>[] = [
  { value: "system", label: "System" },
  { value: "zh-CN", label: "中文" },
  { value: "en", label: "English" },
]

export function SettingsView({ api, config }: { api: KanbanApi | null; config: RuntimeConfig | null }) {
  const { locale, mode, setMode, t } = useI18n()
  const healthQuery = useQuery({
    enabled: Boolean(api),
    queryKey: ["health", config?.apiBaseUrl ?? "pending"],
    queryFn: ({ signal }) => {
      if (!api) throw new Error(t("API client is not ready."))
      return api.health({ signal })
    },
    placeholderData: keepPreviousData,
  })
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
          <InfoRow label={t("board")} value={config?.board ?? "-"} />
          <InfoRow label={t("actor")} value={config?.actor ?? "-"} />
          <InfoRow label={t("api base")} value={config?.apiBaseUrl || t("same-origin")} />
          <InfoRow label={t("database")} value={reportedValue(healthQuery.data?.db_path, t)} />
          <InfoRow label={t("db_fingerprint")} value={reportedValue(healthQuery.data?.db_fingerprint, t)} />
        </div>
        {healthQuery.error ? (
          <Alert className="mt-3 border-destructive/50">
            <AlertTitle className="text-destructive">{t("Server unavailable. Start or check kanban serve.")}</AlertTitle>
            <AlertDescription className="text-destructive">{presentApiError(healthQuery.error, t)}</AlertDescription>
          </Alert>
        ) : null}
      </Card>
    </ScrollArea>
  )
}

function reportedValue(value: string | undefined, t: (key: string) => string) {
  const trimmed = value?.trim()
  return trimmed || t("not reported")
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
