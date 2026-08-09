import { lazy, Suspense, useEffect, useState } from "react"

import type { AppNavigationTarget, OntologyRouteFilters, SignalsRouteFilters } from "../lib/router"
import type { CanonicalBoardSlug } from "../lib/board-slug"
import type { WebRuntimeConfig } from "../lib/runtime"
import { createTranslator } from "../lib/i18n"
import { usePreferences } from "../lib/use-preferences"
import { createHttpTransport } from "../lib/api/http-transport"
import { subscribeBoardSessionTelemetry } from "./board/board-session-registry"
import { telemetryInvalidatesFeature } from "./board-feature-invalidation"
import {
  createSignalsOntologyReadApi,
  resolveSignalsOntologyBoardIdentity,
  type FeatureBoardIdentity,
  type SignalsOntologyReadApi,
} from "../lib/api/signals-ontology-read-model"

const SignalsScreen = lazy(() => import("./signals/SignalsScreen").then((module) => ({ default: module.SignalsScreen })))
const OntologyScreen = lazy(() => import("./ontology/OntologyScreen").then((module) => ({ default: module.OntologyScreen })))

type BoardFeatureRouteProps = {
  readonly runtime: WebRuntimeConfig
  readonly route: FeatureRoute
  readonly onNavigate: (target: AppNavigationTarget) => void | Promise<unknown>
}

export type FeatureRoute = {
  readonly kind: "board"
  readonly boardSlug: CanonicalBoardSlug
  readonly pathname: string
  readonly view: "signals" | "ontology"
  readonly filters?: SignalsRouteFilters | OntologyRouteFilters
}

function featureErrorMessage(error: unknown, fallback: string): string {
  return error instanceof Error && error.message.trim() ? error.message : fallback
}

export function BoardFeatureRoute({ runtime, route, onNavigate }: BoardFeatureRouteProps) {
  const { locale } = usePreferences()
  const t = createTranslator(locale)
  const routeKey = `${runtime.apiBaseUrl}\u0000${runtime.webBasePath}\u0000${runtime.webBuildId}\u0000${runtime.serverVersion}\u0000${runtime.protocolVersion}\u0000${route.boardSlug}`
  const [state, setState] = useState<{ readonly key: string; readonly identity: FeatureBoardIdentity; readonly api: SignalsOntologyReadApi } | null>(null)
  const [error, setError] = useState<{ readonly key: string; readonly value: unknown } | null>(null)
  const [invalidationRevision, setInvalidationRevision] = useState(0)

  useEffect(() => {
    setInvalidationRevision(0)
  }, [routeKey])

  useEffect(() => {
    const controller = new AbortController()
    let active = true
    setState(null)
    setError(null)
    const transport = createHttpTransport(runtime)
    void resolveSignalsOntologyBoardIdentity(runtime, route.boardSlug, { transport, signal: controller.signal }).then((identity) => {
      if (!active || controller.signal.aborted) return
      setState({ key: routeKey, identity, api: createSignalsOntologyReadApi(runtime, { board: route.boardSlug, transport, identity }) })
    }).catch((reason: unknown) => {
      if (active && !controller.signal.aborted) setError({ key: routeKey, value: reason })
    })
    return () => {
      active = false
      controller.abort()
    }
  }, [route.boardSlug, routeKey, runtime])

  useEffect(() => {
    if (state?.key !== routeKey) return
    return subscribeBoardSessionTelemetry(runtime, state.identity.canonicalBoardId, (entry) => {
      if (!telemetryInvalidatesFeature(route.view, entry.type, entry.details?.eventKind)) return
      setInvalidationRevision((revision) => revision + 1)
    })
  }, [route.view, routeKey, runtime, state])

  if (error?.key === routeKey) {
    return (
      <section role="alert" data-testid="board-feature-error">
        <h1>{t("featureLoadError")}</h1>
        <p>{featureErrorMessage(error.value, t("featureBoardIdentityError"))}</p>
        <button type="button" onClick={() => window.location.reload()}>{t("featureRetry")}</button>
      </section>
    )
  }
  if (state?.key !== routeKey) {
    return <section role="status" data-testid="board-feature-loading">{t("featureResolvingBoard")}…</section>
  }

  const filters = route.filters ?? {}
  const navigateFeature = (nextFilters: SignalsRouteFilters | OntologyRouteFilters) => {
    void onNavigate({ kind: "board", boardSlug: route.boardSlug, view: route.view, filters: nextFilters })
  }
  const selectedSignalId = "signal" in filters ? filters.signal ?? null : null
  const closeDetail = () => {
    const next = { ...filters, signal: undefined } as SignalsRouteFilters | OntologyRouteFilters
    void onNavigate({ kind: "board", boardSlug: route.boardSlug, view: route.view, filters: next })
  }

  return (
    <Suspense fallback={<section role="status" data-testid="board-feature-screen-loading">{t("featureLoading")}…</section>}>
      {route.view === "signals" ? (
        <SignalsScreen
          api={state.api}
          boardName={state.identity.name}
          filters={filters as SignalsRouteFilters}
          invalidationRevision={invalidationRevision}
          selectedSignalId={selectedSignalId}
          onFiltersChange={(next) => navigateFeature(next)}
          onSelectSignal={(signalId) => navigateFeature({ ...(filters as SignalsRouteFilters), signal: signalId ?? undefined })}
          onCloseDetail={closeDetail}
        />
      ) : (
        <OntologyScreen
          api={state.api}
          boardName={state.identity.name}
          filters={filters as OntologyRouteFilters}
          invalidationRevision={invalidationRevision}
          selectedSignalId={selectedSignalId}
          onFiltersChange={(next) => navigateFeature(next)}
          onSelectSignal={(signalId) => navigateFeature({ ...(filters as OntologyRouteFilters), signal: signalId ?? undefined })}
          onCloseDetail={closeDetail}
          onLifecycleAction={async (action, signalId, reason) => {
            await state.api.createLabelOntologyLifecycleAction(action, signalId, reason)
          }}
        />
      )}
    </Suspense>
  )
}
